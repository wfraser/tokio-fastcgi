extern crate tokio_fastcgi;
use tokio_fastcgi::*;

extern crate env_logger;
extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_uds;

use futures::{future, Future, Stream};
use futures::sync::oneshot;
use tokio_core::reactor::Core;
use tokio_proto::BindServer;
use tokio_uds::*;

use std::fs;
use std::io;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn umask(mask: u32) -> u32 {
    extern "system" { fn umask(mask: u32) -> u32; }
    unsafe { umask(mask) }
}

struct CountdownHandler(i32);

impl FastcgiRequestHandler for CountdownHandler {
    fn call(&self, request: FastcgiRequest) -> Box<dyn Future<Item=(), Error=io::Error>> {
        let start = self.0;
        let mut headers_response = request.response();
        headers_response.set_header("Content-Type", "text/plain");
        let x = headers_response.send_headers()
            .and_then(move |mut body_response| {
                println!("beginning countdown from {}", start);

                // Add a bunch of invisible characters to the output to prevent browsers from
                // buffering the whole thing.
                // U+FEFF works great because as a Zero-Width-Non-Breaking-Space it is almost
                // totally invisible.
                const PADDING_LEN: usize = 100;
                body_response.buffer.reserve(PADDING_LEN * 3); // U+FEFF = 3 bytes in UTF-8
                for _ in 0..PADDING_LEN {
                    body_response.buffer.extend_from_slice("\u{FEFF}".as_bytes());
                }

                body_response.buffer.append(
                    &mut format!("Counting down from {}!\n", start).into_bytes());
                body_response.flush()
            }).and_then(move |body_response| {
                future::loop_fn((start, body_response), |(i, mut body_response)| -> Box<dyn Future<Item=future::Loop<_,_>, Error=_>> {
                    if i == 0 {
                        Box::new(future::ok(future::Loop::Break(body_response)))
                    } else {
                        println!("{}", i);
                        body_response.buffer.append(&mut format!("{}\n", i).into_bytes());

                        Box::new(body_response.flush()
                            .and_then(move |next| {
                                let (tx, rx) = oneshot::channel::<()>();

                                thread::spawn(move || {
                                    thread::sleep(Duration::from_millis(1000));
                                    tx.send(()).unwrap();
                                });

                                rx.map_err(|e| panic!("{}", e))
                                    .map(move |()| future::Loop::Continue((i - 1, next)))
                            }))
                    }
                })
            }).and_then(|mut body_response| {
                println!("Done!");
                body_response.buffer.extend_from_slice(b"Done!\n");
                body_response.finish()
            });

        Box::new(x)
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
    let listener = UnixListener::bind(filename).expect("failed to bind socket");

    let handler = Arc::new(CountdownHandler(10));

    let srv = listener.incoming().for_each(|socket| {
        println!("New connection.");
        let service = FastcgiService::new(remote.clone(), handler.clone());
        let proto = FastcgiProto::default();
        proto.bind_server(&handle, socket, service);
        Ok(())
    });

    reactor.run(srv).expect("failed to run the server");
}
