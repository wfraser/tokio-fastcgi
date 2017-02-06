extern crate tokio_fastcgi;
use tokio_fastcgi::*;

extern crate env_logger;
extern crate futures;
#[macro_use] extern crate log;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;
extern crate tokio_uds;

use futures::{future, stream, Future, Stream, Sink};
use tokio_core::reactor::{Core, Handle};
use tokio_core::io::{Io, Codec, EasyBuf, Framed};
use tokio_proto::BindServer;
use tokio_proto::streaming::{Message, Body};
use tokio_proto::streaming::multiplex::*;
use tokio_service::Service;
use tokio_uds::*;

use std::fs;
use std::io;

fn umask(mask: u32) -> u32 {
    extern "system" { fn umask(mask: u32) -> u32; }
    unsafe { umask(mask) }
}

//
// Experimental code that will all go in the main crate once it's working:
//

//

struct FastcgiMultiplexedPipelinedCodec {
    inner: FastcgiLowlevelCodec,
}

impl Codec for FastcgiMultiplexedPipelinedCodec {
    type In = Frame<FastcgiRecord, FastcgiRecord, io::Error>;
    type Out = Frame<FastcgiRecord, FastcgiRecord, io::Error>;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Self::In>, io::Error> {
        match self.inner.decode(buf) {
            Ok(Some(record)) => {
                match record.body {
                    FastcgiRecordBody::BeginRequest(_) => {
                        debug!("got BeginRequest, sending header chunk");
                        Ok(Some(Frame::Message {
                            id: record.request_id as RequestId,
                            message: record,
                            body: true,
                            solo: false
                        }))
                    },
                    FastcgiRecordBody::Stdin(ref buf) if buf.len() == 0 => {
                        debug!("stdin is done; sending empty body chunk");
                        Ok(Some(Frame::Body {
                            id: record.request_id as RequestId,
                            chunk: None,
                        }))
                    },
                    _ => {
                        if let FastcgiRecordBody::Data(_) = record.body {
                            warn!("FCGI_DATA not supported");
                        }
                        debug!("sending body chunk");
                        Ok(Some(Frame::Body {
                            id: record.request_id as RequestId,
                            chunk: Some(record)
                        }))
                    }
                }
            },
            Ok(None) => {
                debug!("got None from underlying codec");
                Ok(None)
            },
            Err(e) => {
                error!("got error from underlying codec: {}", e);
                Err(e)
            }
        }
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
        match msg {
            Frame::Message { id, message, body, solo } => {
                debug!("encoding message: {} {:?} body={:?} solo={:?}", id, message, body, solo);
                self.inner.encode(message, buf)
            },
            Frame::Body { id, chunk } => {
                debug!("encoding body: {} {:?}", id, chunk);
                if let Some(rec) = chunk {
                    self.inner.encode(rec, buf)
                } else {
                    Ok(())
                }
            },
            Frame::Error { id, error } => {
                error!("error: {} {}", id, error);
                Err(error)
            }
        }
    }
}

//

struct FastcgiService {
    core_handle: Handle,
}

impl Service for FastcgiService {
    type Request = Message<FastcgiRecord, Body<FastcgiRecord, io::Error>>;
    type Response = Message<FastcgiRecord, Body<FastcgiRecord, io::Error>>;
    type Error = io::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {

        let (id, input_record_stream) = match req {
            Message::WithoutBody(_) => {
                // All requests take up more than one record, so we're not using these.
                panic!("unexpected WithoutBody response: {:?}", req);
            },
            Message::WithBody(head, body) => {
                debug!("Got request headers!");
                debug!("head = {:?}", head);
                debug!("body = {:?}", body);
                (head.request_id, body)
            }
        };

        // Send something now, then read the rest of the records and do the rest of the response in
        // the spawned function.
        let data = "X-Powered-By: tokio-fastcgi/0.1\r\n";
        let out = EasyBuf::from(Vec::from(data.as_bytes()));

        let (body_sender, body) = Body::pair();

        let resp = Message::WithBody(
            FastcgiRecord {
                request_id: id,
                body: FastcgiRecordBody::Stdout(out),
            },
            body);

        self.core_handle.spawn_fn(move || {
            input_record_stream.collect()
                .then(move |records| {
                    debug!("{:?}", records);

                    let data = concat!(
                        "Content-Type: text/plain\r\n",
                        "\r\n",
                        "Hello!"
                    );
                    let buf = EasyBuf::from(Vec::from(data.as_bytes()));

                    body_sender.send(Ok(FastcgiRecord {
                        request_id: id,
                        body: FastcgiRecordBody::Stdout(buf)
                    }))
                })
                .then(move |sender_result| {
                    let end_records = vec![
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

                    debug!("sending end records");
                    sender_result.unwrap().send_all(stream::iter(end_records))
                })
                .then(|_| {
                    debug!("done sending end records");
                    future::ok(())
                })
        });

        future::ok(resp).boxed()
    }
}

//

struct FastcgiProto;

impl<IO: Io + 'static> ServerProto<IO> for FastcgiProto {
    type Request = FastcgiRecord;
    type RequestBody = FastcgiRecord;
    type Response = FastcgiRecord;
    type ResponseBody = FastcgiRecord;
    type Error = io::Error;

    type Transport = Framed<IO, FastcgiMultiplexedPipelinedCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: IO) -> Self::BindTransport {
        let codec = FastcgiMultiplexedPipelinedCodec {
            inner: FastcgiLowlevelCodec,
        };
        Ok(io.framed(codec))
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

    umask(0);
    let listener = UnixListener::bind(filename, &reactor.handle()).expect("failed to bind socket");

    let srv = listener.incoming().for_each(move |(socket, _addr)| {
        println!("{:#?}", socket);

        let service = FastcgiService {
            core_handle: handle.clone(),
        };
        let proto = FastcgiProto;
        proto.bind_server(&handle, socket, service);

        Ok(())
    });

    reactor.run(srv).expect("failed to run the server");
}
