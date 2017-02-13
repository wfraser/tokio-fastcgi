extern crate tokio_fastcgi;
use tokio_fastcgi::*;

extern crate env_logger;
extern crate futures;
#[macro_use] extern crate log;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;
extern crate tokio_uds;

use futures::{future, BoxFuture, Stream};
use tokio_core::reactor::Core;
use tokio_proto::BindServer;
use tokio_uds::*;

use std::fs;
use std::io;
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn umask(mask: u32) -> u32 {
    extern "system" { fn umask(mask: u32) -> u32; }
    unsafe { umask(mask) }
}

struct HelloHandler {
    request_count: AtomicUsize,
}

impl HelloHandler {
    pub fn new() -> HelloHandler {
        HelloHandler {
            request_count: AtomicUsize::new(1),
        }
    }
}

impl FastcgiRequestHandler for HelloHandler {
    fn call(&self, request: FastcgiRequest) -> BoxFuture<FastcgiResponse, io::Error> {
        println!("making the response");

        let mut response = FastcgiResponse::new();
        response.header("X-Powered-By", "tokio_fastcgi/0.1");
        response.header("Content-Type", "text/plain");

        let count = self.request_count.fetch_add(1, Ordering::SeqCst);

        let body = format!("Hello from {:?}: {}\n", request.params["REQUEST_URI"], count);
        response.body.append(&mut body.into_bytes());

        Box::new(future::ok(response))
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

    let handler = Arc::new(HelloHandler::new());

    let srv = listener.incoming().for_each(|(socket, _addr)| {
        println!("New connection: fd {}", socket.as_raw_fd());

        let service = FastcgiService::new(remote.clone(), handler.clone());

        let proto = FastcgiProto;
        proto.bind_server(&handle, socket, service);

        Ok(())
    });

    reactor.run(srv).expect("failed to run the server");
}
