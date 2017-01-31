extern crate tokio_fastcgi;
use tokio_fastcgi::*;

extern crate byteorder;
extern crate env_logger;
extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;
extern crate tokio_uds;

use futures::{future, Future, Stream};
use tokio_core::reactor::Core;
use tokio_proto::BindServer;
use tokio_proto::streaming::{Body, Message};
use tokio_service::Service;
use tokio_uds::*;

use std::fs;
use std::io;

struct HelloService;

impl Service for HelloService {
    type Request = Message<FastcgiRecord, Body<FastcgiRecord, io::Error>>;
    type Response = Message<FastcgiRecord, Body<FastcgiRecord, io::Error>>;
    type Error = io::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let mut resp = Message::WithoutBody(FastcgiRecord {
            record_type: RecordType::UnknownType,
            request_id: 0,
            content: b"hello".to_vec(),
        });

        match req {
            Message::WithoutBody(record) => {
                println!("withoutbody: {:?}", record);
                println!("  {:?}", String::from_utf8_lossy(record.content.as_slice()));
                resp.request_id = record.request_id;
                future::done(Ok(resp)).boxed()
            },
            Message::WithBody(head, body) => {
                println!("withbody: head: {:?}", head);
                println!("withbody: body: {:?}", body);

                resp.request_id = head.request_id;

                let resp = body
                    .for_each(|record| {
                        println!("{:?}", record);
                        match record.record_type {
                            //TODO
                        }

                        Ok(())
                    })
                    .map(move |_| resp);

                resp.boxed()
            }
        }
    }
}

pub fn main() {
    env_logger::init().unwrap();

    let filename = "hello.sock";

    let mut reactor = Core::new().expect("failed to create reactor core");

    if let Err(e) = fs::remove_file(filename) {
        if e.kind() != io::ErrorKind::NotFound {
            panic!("failed to remove existing socket file {:?}: {}", filename, e);
        }
    }

    let listener = UnixListener::bind(filename, &reactor.handle()).expect("failed to bind socket");

    let handle = reactor.handle();

    let srv = listener.incoming().for_each(move |(socket, _addr)| {
        println!("{:#?}", socket);

        let proto = FastcgiProto;
        proto.bind_server(&handle, socket, HelloService);
        Ok(())
    });

    reactor.run(srv).expect("failed to run the server");
}
