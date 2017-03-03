extern crate tokio_fastcgi;
use tokio_fastcgi::*;

extern crate env_logger;
extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_uds;

use futures::{future, BoxFuture, Future, Stream};
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
    fn call(&self, request: FastcgiRequest) -> BoxFuture<(), io::Error> {
        let start = self.0;
        let mut headers_response = request.response();
        headers_response.set_header("Content-Type", "text/plain");
        headers_response.send_headers()
            .and_then(move |mut body_response| {
                println!("beginning countdown from {}", start);
                body_response.buffer.append(
                    &mut format!("Counting down from {}!\n", start).into_bytes());
                body_response.flush()
            }).and_then(move |body_response| {
                future::loop_fn((start, body_response), |(i, mut body_response)| {
                    if i == 0 {
                        future::ok(future::Loop::Break(body_response)).boxed()
                    } else {
                        println!("{}", i);
                        body_response.buffer.append(&mut format!("{}\n", i).into_bytes());

                        body_response.flush()
                            .and_then(move |next| {
                                let (tx, rx) = oneshot::channel::<()>();

                                thread::spawn(move || {
                                    thread::sleep(Duration::from_millis(1000));
                                    tx.complete(());
                                });

                                rx.map_err(|e| panic!("{}", e))
                                    .map(move |()| future::Loop::Continue((i - 1, next)))
                            }).boxed()
                    }
                })
            }).and_then(|mut body_response| {
                println!("Done!");
                body_response.buffer.extend_from_slice(b"Done!\n");
                body_response.finish()
            }).boxed()
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

    let handler = Arc::new(CountdownHandler(10));

    let srv = listener.incoming().for_each(|(socket, _addr)| {
        println!("New connection.");
        let service = FastcgiService::new(remote.clone(), handler.clone());
        let proto = FastcgiProto::default();
        proto.bind_server(&handle, socket, service);
        Ok(())
    });

    reactor.run(srv).expect("failed to run the server");
}