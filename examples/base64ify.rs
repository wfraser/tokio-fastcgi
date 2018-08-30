//! This is a silly demo that takes an uploaded file and returns it as Base64.
//! It demonstrates FastCGI's ability to read and write data of arbitrary lengths in chunks.

extern crate tokio_fastcgi;
use tokio_fastcgi::*;

extern crate env_logger;
extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_uds;

use futures::{Future, Stream};
use tokio_core::reactor::Core;
use tokio_proto::BindServer;
use tokio_uds::*;

use std::fs;
use std::io;
use std::os::unix::io::AsRawFd;
use std::sync::Arc;

fn umask(mask: u32) -> u32 {
    extern "system" { fn umask(mask: u32) -> u32; }
    unsafe { umask(mask) }
}

fn write_base64_digit(six_bits: u8, out: &mut Vec<u8>) {
    let c = match six_bits {
        0...25 => b'A' + six_bits,
        26...51 => b'a' + six_bits - 26,
        52...61 => b'0' + six_bits - 52,
        62 => b'+',
        63 => b'/',
        _ => panic!("input out of range")
    };
    out.push(c);
}

fn write_base64(bytes: &[u8], out: &mut Vec<u8>) {
    let a = (bytes[0] & 0b1111_1100) >> 2;
    let mut b = (bytes[0] & 0b11) << 4;
    write_base64_digit(a, out);

    if bytes.len() == 1 {
        write_base64_digit(b, out);
        out.extend_from_slice(b"==");
    } else {
        b |= (bytes[1] & 0b1111_0000) >> 4;
        let mut c = (bytes[1] & 0b1111) << 2;
        write_base64_digit(b, out);

        if bytes.len() == 2 {
            write_base64_digit(c, out);
            out.push(b'=');
        } else {
            c |= (bytes[2] & 0b1100_0000) >> 6;
            let d = bytes[2] & 0b11_1111;
            write_base64_digit(c, out);
            write_base64_digit(d, out);

            if bytes.len() > 3 {
                panic!("write_base64 input too large");
                // If we wanted to handle the general case, we would do this (yay tail recursion):
                //write_base64(&bytes[3..], out);
            }
        }
    }
}

struct Base64ifyHandler;

impl FastcgiRequestHandler for Base64ifyHandler {
    fn call(&self, request: FastcgiRequest) -> Box<Future<Item=(), Error=io::Error>> {
        let mut headers_response = request.response();
        headers_response.set_header("Content-Type", "text/plain");

        struct State {
            column: u32,
            partial: Option<Vec<u8>>,
        }

        let state = State {
            column: 0,
            partial: None,
        };

        Box::new(headers_response.send_headers()
            .and_then(move |body_response| {
                request.body.fold(
                    (body_response, state),
                    move |(mut body_response, mut state), mut input_buf|
                {
                    println!("input record buffer size = {}", input_buf.len());

                    // Take the partial data from the last iteration (if any) and fill it up with
                    // data from this iteration.
                    if let Some(mut partial) = state.partial.take() {
                        // how many bytes round it out?
                        let fill_count = std::cmp::min(
                            input_buf.len(),
                            match partial.len() {
                                1 => 2,
                                2 => 1,
                                _ => unreachable!(),
                            });
                        partial.extend_from_slice(&input_buf.split_to(fill_count));
                        if partial.len() == 3 {
                            write_base64(&partial, body_response.buffer.as_mut());
                            state.column += 4;
                        } else {
                            // still need more; put it back
                            state.partial = Some(partial);
                        }
                    }

                    if state.column == 76 {
                        body_response.buffer.extend_from_slice(b"\r\n");
                        state.column = 0;
                    }

                    for chunk in input_buf.chunks(3) {
                        if chunk.len() == 3 {
                            write_base64(chunk, body_response.buffer.as_mut());
                            state.column += 4;

                            if state.column == 76 {
                                body_response.buffer.extend_from_slice(b"\r\n");
                                state.column = 0;
                            }
                        } else {
                            println!("saving partial of {} bytes", chunk.len());
                            state.partial = Some(chunk.to_vec());
                        }
                    }

                    body_response.flush().map(move |x| (x, state))
                })
            }).and_then(move |(mut body_response, mut state)| {
                // Any partial data left at the end needs to be written out. It'll end up with '='
                // padding.
                if let Some(partial) = state.partial.take() {
                    println!("writing partial of {} bytes", partial.len());
                    write_base64(&partial, body_response.buffer.as_mut());
                }

                body_response.buffer.extend_from_slice(b"\r\n");

                println!("done");
                body_response.finish()
            }))
    }
}

fn main() {
    env_logger::init().unwrap();

    let filename = "hello.sock";
    if let Err(e) = fs::remove_file(filename) {
        if e.kind() != io::ErrorKind::NotFound {
            panic!("failed to remove existing socket file {:?}: {}", filename, e);
        }
    }

    let mut reactor = Core::new().unwrap();
    let handle = reactor.handle();
    let remote = reactor.remote();

    umask(0);
    let listener = UnixListener::bind(filename, &reactor.handle()).expect("failed to bind socket");

    let srv = listener.incoming().for_each(|(socket, _addr)| {
        println!("New connection: fd {}", socket.as_raw_fd());

        let service = FastcgiService::new(remote.clone(), Arc::new(Base64ifyHandler));

        let proto = FastcgiProto::default();
        proto.bind_server(&handle, socket, service);

        Ok(())
    });

    reactor.run(srv).expect("failed to run the server");
}
