extern crate tokio_fastcgi;
use tokio_fastcgi::*;

extern crate env_logger;
extern crate futures;
#[macro_use] extern crate log;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;
extern crate tokio_uds;

use futures::{future, stream, BoxFuture, Future, Stream, Sink};
use tokio_core::reactor::{Core, Remote};
use tokio_core::io::{EasyBuf, EasyBufMut};
use tokio_proto::BindServer;
use tokio_proto::streaming::{Message, Body};
use tokio_service::Service;
use tokio_uds::*;

use std::collections::HashMap;
use std::fs;
use std::io;
use std::os::unix::io::AsRawFd;

fn umask(mask: u32) -> u32 {
    extern "system" { fn umask(mask: u32) -> u32; }
    unsafe { umask(mask) }
}

//
// Experimental code that will all go in the main crate once it's working:
//

struct FastcgiService<F> {
    reactor_handle: Remote,
    handler: F,
}

impl<F> FastcgiService<F>
        where F: Fn(BoxFuture<FastcgiRequest, io::Error>)
                -> BoxFuture<FastcgiResponse, io::Error>
{
    pub fn new(reactor_handle: Remote, handler: F) -> FastcgiService<F> {
        FastcgiService {
            reactor_handle: reactor_handle,
            handler: handler,
        }
    }
}

pub struct FastcgiRequest {
    pub id: u16,
    pub role: Role,
    pub params: HashMap<String, String>,
    pub body: Body<FastcgiRecord, io::Error>,
}

pub struct FastcgiResponse {
    pub body: Vec<u8>, // TODO: change this to be a channel
    headers: HashMap<String, String>,
}

impl FastcgiResponse {
    pub fn new() -> FastcgiResponse {
        FastcgiResponse {
            body: vec![],
            headers: HashMap::new(),
        }
    }

    pub fn header<K: Into<String>, V: Into<String>>(&mut self, name: K, value: V) {
        self.headers.insert(name.into(), value.into());
    }
}

fn write_headers(buf: &mut EasyBufMut, headers: &HashMap<String, String>) {
    for (ref key, ref value) in headers {
        buf.extend_from_slice(key.as_bytes());
        buf.extend_from_slice(b": ");
        buf.extend_from_slice(value.as_bytes());
        buf.extend_from_slice(b"\r\n");
    }
}

impl<F> Service for FastcgiService<F>
        where F: Fn(BoxFuture<FastcgiRequest, io::Error>)
                -> BoxFuture<FastcgiResponse, io::Error>
{
    type Request = Message<FastcgiRecord, Body<FastcgiRecord, io::Error>>;
    type Response = Message<FastcgiRecord, Body<FastcgiRecord, io::Error>>;
    type Error = io::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {

        // Conceptually:
        //
        // Read records from the body stream until the empty Params record,
        //  building up the params hash map.
        // Then pass the hash map and the remaining stream to the handler function,
        //  which returns a response with body stream.

        let (id, begin_request, input_record_stream): (u16, BeginRequest, Body<FastcgiRecord, io::Error>) = match req {
            Message::WithBody(
                FastcgiRecord {
                    request_id,
                    body: FastcgiRecordBody::BeginRequest(begin_request)
                },
                body
            ) => {
                debug!("Got BeginRequest record: {:?}", begin_request);
                (request_id, begin_request, body)
            },
            Message::WithBody(record, _body) => {
                panic!("unexpected first record instead of BeginRequest: {:?}", record);
            },
            Message::WithoutBody(_) => {
                // All requests take up more than one record, so we're not using these.
                panic!("unexpected WithoutBody response: {:?}", req);
            },
        };

        let params_map = HashMap::<String, String>::new();

        let stream_process = StreamProcess::new(input_record_stream, params_map, move |record, params_map| {
            match record.body {
                FastcgiRecordBody::Params(ref params) => {
                    if params.is_empty() {
                        debug!("done reading params");
                        true
                    } else {
                        debug!("consuming a params record");
                        for &(ref name_buf, ref value_buf) in params {
                            let name = String::from_utf8_lossy(name_buf.as_slice()).into_owned();
                            let value = String::from_utf8_lossy(value_buf.as_slice()).into_owned();
                            params_map.insert(name, value);
                        }
                        false
                    }
                },
                _ => panic!("unexpected record while reading params: {:?}", record)
            }
        });

        let reactor_handle = self.reactor_handle.clone();
        let keep_connection = begin_request.keep_connection;

        let request_future = stream_process.and_then(move |(body_record_stream, params)| {
            Ok(FastcgiRequest {
                id: id,
                role: begin_request.role,
                params: params,
                body: body_record_stream,
            })
        });

        Box::new((self.handler)(request_future.boxed())
            .and_then(move |response| {
                let mut out = EasyBuf::new();
                {
                    let mut buf_mut = out.get_mut();
                    write_headers(&mut buf_mut, &response.headers);
                    buf_mut.extend_from_slice(b"\r\n");
                    buf_mut.extend_from_slice(response.body.as_slice());
                }

                let (body_sender, body) = Body::pair();

                let resp = Message::WithBody(
                    FastcgiRecord {
                        request_id: id,
                        body: FastcgiRecordBody::Stdout(out),
                    },
                    body);

                reactor_handle.spawn(move |_| {
                    let mut end_records = vec![
                        Ok(Ok(FastcgiRecord {
                            request_id: id,
                            body: FastcgiRecordBody::Stdout(EasyBuf::new()),
                        })),
                        Ok(Ok(FastcgiRecord {
                            request_id: id,
                            body: FastcgiRecordBody::Stderr(EasyBuf::new()),
                        })),
                        Ok(Ok(FastcgiRecord {
                            request_id: id,
                            body: FastcgiRecordBody::EndRequest(EndRequest {
                                app_status: 0,
                                protocol_status: ProtocolStatus::RequestComplete,
                            })
                        })),
                    ];

                    if !keep_connection {
                        // HACK HACK HACK
                        // The only way to drop the connection (as far as I can tell) is to send an
                        // error here.
                        end_records.push(Ok(Err(io::Error::new(
                            io::ErrorKind::Other,
                            "tokio-fastcgi forcing connection drop"))));
                    }

                    debug!("sending end records");
                    body_sender.send_all(stream::iter(end_records))
                        .then(|_| {
                            debug!("done sending end records");
                            future::ok(())
                        })
                });

                Ok(resp)
            }))
    }
}

//

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

    let srv = listener.incoming().for_each(move |(socket, _addr)| {
        println!("New connection: fd {}", socket.as_raw_fd());

        let service = FastcgiService::new(remote.clone(), move |stream_processor| {
            println!("got a request");
            let resp_future = stream_processor.and_then(|request| {
                println!("making the response");

                let mut response = FastcgiResponse::new();
                response.header("X-Powered-By", "tokio_fastcgi/0.1");
                response.header("Content-Type", "text/plain");

                let body = format!("Hello from {:?}!\n", request.params["REQUEST_URI"]);
                response.body.append(&mut body.into_bytes());

                Box::new(future::ok(response))
            });
            Box::new(resp_future)
        });

        let proto = FastcgiProto;
        proto.bind_server(&handle, socket, service);

        Ok(())
    });

    reactor.run(srv).expect("failed to run the server");
}
