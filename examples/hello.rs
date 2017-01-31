extern crate tokio_fastcgi;
use tokio_fastcgi::*;

extern crate byteorder;
extern crate env_logger;
extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;
extern crate tokio_uds;

use byteorder::{ByteOrder, NetworkEndian};
use futures::{future, Future, Stream};
use tokio_core::reactor::Core;
use tokio_proto::BindServer;
use tokio_proto::streaming::{Body, Message};
use tokio_service::Service;
use tokio_uds::*;

use std::fs;
use std::io;

struct HelloService;

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
                match head.record_type {
                    RecordType::BeginRequest => {
                        let body: BeginRequestBody = unsafe {
                            from_bytes(&head.content)
                        };
                        println!("  head body = {:?}", body);
                    },
                    _ => ()
                }

                println!("withbody: body: {:?}", body);

                resp.request_id = head.request_id;
                let resp = body
                    .for_each(|record| {
                        println!("  {:?}", record);
                        match record.record_type {
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
                            }
                            _ => ()
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
