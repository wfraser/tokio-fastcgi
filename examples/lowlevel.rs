extern crate tokio_fastcgi;
use tokio_fastcgi::*;

extern crate byteorder;
extern crate env_logger;
extern crate futures;
extern crate tokio_core;
extern crate tokio_uds;

use byteorder::{ByteOrder, NetworkEndian};
use futures::{future, Stream};
use tokio_core::reactor::Core;
use tokio_core::io::Io;
use tokio_uds::*;

use std::fs;
use std::io;

fn read_len(idx: &mut usize, bytes: &[u8]) -> usize {
    let len: usize;
    if bytes[*idx] < 0x80 {
        len = bytes[*idx] as usize;
        *idx += 1;
    } else {
        len = NetworkEndian::read_u32(&bytes[*idx..]) as usize;
        *idx += 4;
    }
    len
}

fn print_record(record: FastcgiRecord) {
    println!("request id: {}", record.request_id);
    println!("request type: {:?}", record.record_type);
    println!("content length: {}", record.content.len());
    match record.record_type {
        RecordType::BeginRequest => {
            let body: BeginRequestBody = unsafe {
                from_bytes(&record.content)
            };
            println!("  {:?}", body);
        },
        RecordType::Params => {
            let mut idx = 0;
            loop {
                if idx == record.content.len() {
                    break;
                }
                let name_len = read_len(&mut idx, &record.content);
                let value_len = read_len(&mut idx, &record.content);
                let mut name = Vec::with_capacity(name_len);
                let mut value = Vec::with_capacity(value_len);
                name.extend_from_slice(&record.content[idx .. idx + name_len]);
                idx += name_len;
                value.extend_from_slice(&record.content[idx .. idx + value_len]);
                idx += value_len;
                println!("  {} = {}",
                         String::from_utf8_lossy(&name),
                         String::from_utf8_lossy(&value));
            }
        },
        _ => (),
    }
}

fn main() {
    env_logger::init().unwrap();

    let filename = "hello.sock";

    let mut reactor = Core::new().expect("failed to create reactor core");

    if let Err(e) = fs::remove_file(filename) {
        if e.kind() != io::ErrorKind::NotFound {
            panic!("failed to remove existing socket file {:?}: {}", filename, e);
        }
    }

    let listener = UnixListener::bind(filename, &reactor.handle()).expect("failed to bind socket");

    let srv = listener.incoming().for_each(move |(socket, _addr)| {
        println!("{:#?}", socket);

        socket.framed(FastcgiLowlevelCodec)
            .take_while(|record| future::ok(!record.content.is_empty()))
            .for_each(|record| {
                print_record(record);
                Ok(())
            })
    });

    reactor.run(srv).expect("failed to run the server");
}
