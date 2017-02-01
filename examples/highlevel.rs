extern crate tokio_fastcgi;
use tokio_fastcgi::*;

extern crate env_logger;
extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;
extern crate tokio_uds;

use futures::{future, Future, Stream};
use tokio_core::reactor::Core;
use tokio_core::io::Io;
use tokio_proto::BindServer;
use tokio_service::Service;
use tokio_uds::*;

use std::fs;
use std::io;

struct FastcgiService;

impl Service for FastcgiService {
    type Request = FastcgiRequest;
    type Response = FastcgiResponse;
    type Error = io::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        future::ok(FastcgiResponse {
            request_id: req.request_id,
            app_status: 0,
            protocol_status: 0,
            body: b"hello".to_vec(),
            //body: future::ok(b"hello".to_vec()).boxed(),
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

    let mut reactor = Core::new().expect("failed to create reactor core");
    let handle = reactor.handle();

    let listener = UnixListener::bind(filename, &reactor.handle()).expect("failed to bind socket");

    let srv = listener.incoming().for_each(move |(socket, _addr)| {
        println!("{:#?}", socket);

        FastcgiProto.bind_server(&handle, socket.framed(FastcgiLowlevelCodec), FastcgiService);

        Ok(())
    });

    reactor.run(srv).expect("failed to run the server");
}
