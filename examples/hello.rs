extern crate tokio_fastcgi;
use tokio_fastcgi::*;

extern crate env_logger;
extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_uds;

use futures::{BoxFuture, Future, Stream};
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
    fn call(&self, request: FastcgiRequest) -> BoxFuture<(), io::Error> {
        let mut headers_response = request.response();
        headers_response.set_header("Content-Type", "text/plain");

        let count = self.request_count.fetch_add(1, Ordering::SeqCst);

        headers_response.send_headers()
            .and_then(move |mut body_response| {
                println!("making the response");
                let body = format!("Hello from {:?}: {}\n", request.params["REQUEST_URI"], count);
                body_response.buffer.append(&mut body.into_bytes());
                body_response.finish()
            })
            .boxed()
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

        let proto = FastcgiProto::default();
        proto.bind_server(&handle, socket, service);

        Ok(())
    });

    reactor.run(srv).expect("failed to run the server");
}
