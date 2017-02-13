use super::super::*;

use futures::{future, stream, Future, Sink};
use tokio_core::reactor::Remote;
use tokio_core::io::EasyBuf;
use tokio_proto::streaming::{Message, Body};
use tokio_service::Service;

use std::collections::HashMap;
use std::io;
use std::sync::Arc;

pub struct FastcgiService<H: FastcgiRequestHandler + 'static> {
    reactor_handle: Remote,
    handler: Arc<H>,
}

impl<H: FastcgiRequestHandler + 'static> FastcgiService<H> {
    pub fn new(reactor_handle: Remote, handler: Arc<H>) -> FastcgiService<H> {
        FastcgiService {
            reactor_handle: reactor_handle,
            handler: handler,
        }
    }
}

fn invalid_data<T: Into<String>>(msg: T) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

impl<H: FastcgiRequestHandler + 'static> Service for FastcgiService<H> {
    type Request = Message<FastcgiRecord, Body<FastcgiRecord, io::Error>>;
    type Response = Message<FastcgiRecord, Body<FastcgiRecord, io::Error>>;
    type Error = io::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {

        // Read records from the body stream until the empty Params record, building up the params
        // hash map.
        // Then pass the hash map and the remaining stream to the handler function, which returns
        // a response with body stream.

        let (id, begin_request, input_record_stream):
                (u16, BeginRequest, Body<FastcgiRecord, io::Error>) = match req {
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
                let msg = format!("unexpected first record instead of BeginRequest: {:?}", record);
                error!("{}", msg);
                return future::err(invalid_data(msg)).boxed();
            },
            Message::WithoutBody(_) => {
                let msg = format!("unexpected WithoutBody response: {:?}", req);
                error!("{}", msg);
                return future::err(invalid_data(msg)).boxed();
            },
        };

        let params_map = HashMap::<String, String>::new();

        let stream_process = StreamProcess::new(
            input_record_stream,
            params_map,
            move |record, params_map| {
                match record.body {
                    FastcgiRecordBody::Params(ref params) => {
                        if params.is_empty() {
                            debug!("done reading params");
                            true
                        } else {
                            debug!("consuming a params record");
                            for &(ref name_buf, ref value_buf) in params {
                                let name = String::from_utf8_lossy(name_buf.as_slice())
                                                  .into_owned();
                                let value = String::from_utf8_lossy(value_buf.as_slice())
                                                  .into_owned();
                                params_map.insert(name, value);
                            }
                            false
                        }
                    }
                    _ => {
                        let msg = format!("unexpected record while reading params: {:?}", record);
                        error!("{}", msg);
                        false
                    }
                }
            }
        );

        let reactor_handle = self.reactor_handle.clone();

        let request_future = stream_process.and_then(move |(body_record_stream, params)| {
            macro_rules! param {
                ($name:expr) => {
                    params.get($name)
                          .map(|s| s.as_str())
                          .unwrap_or(concat!("<no ", $name, " set!>"))
                }
            }

            info!("remote {:?} -> request for {:?}", param!("REMOTE_ADDR"), param!("REQUEST_URI"));

            Ok(FastcgiRequest {
                role: begin_request.role,
                params: params,
                body: body_record_stream,
            })
        });

        let handler = self.handler.clone();
        let response_future = request_future
            .and_then(move |request| handler.call(request))
            .and_then(move |response| {

                // TODO: need to split the response into multiple records if it is greater than
                // 0xFFFF in length.

                let mut out = EasyBuf::with_capacity(response.body.len());
                {
                    let mut buf_mut = out.get_mut();
                    for (ref key, ref value) in response.headers {
                        buf_mut.extend_from_slice(key.as_bytes());
                        buf_mut.extend_from_slice(b": ");
                        buf_mut.extend_from_slice(value.as_bytes());
                        buf_mut.extend_from_slice(b"\r\n");
                    }
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
                    body_sender.send_all(stream::iter(end_records))
                        .then(|_| {
                            debug!("done sending end records");
                            future::ok(())
                        })
                });

                Ok(resp)
            });
        Box::new(response_future)
    }
}

pub struct FastcgiRequest {
    pub role: Role,
    pub params: HashMap<String, String>,
    pub body: Body<FastcgiRecord, io::Error>,
}

pub struct FastcgiResponse {
    pub body: Vec<u8>,  // TODO: change this to be a channel
    headers: HashMap<String, String>,
}

impl FastcgiResponse {
    pub fn new() -> FastcgiResponse {
        let mut headers = HashMap::new();
        headers.insert(
            "X-Powered-By".to_owned(),
            concat!("tokio-fastcgi/", env!("CARGO_PKG_VERSION")).to_owned());
        FastcgiResponse {
            body: Vec::new(),
            headers: headers,
        }
    }

    pub fn set_header<K: Into<String>, V: Into<String>>(&mut self, name: K, value: V) {
        self.headers.insert(name.into(), value.into());
    }

    pub fn clear_header(&mut self, name: &str) {
        self.headers.remove(name);
    }
}
