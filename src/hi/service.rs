use super::super::*;

use futures::{future, stream, Future, Sink, Stream};
use futures::sync::mpsc;
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
        let (response_sender, response_receiver) = mpsc::channel::<FastcgiRecord>(1);

        let request_future = stream_process.and_then(move |(body_record_stream, params)| {
            macro_rules! param {
                ($name:expr) => {
                    params.get($name)
                          .map(|s| s.as_str())
                          .unwrap_or(concat!("<no ", $name, " set!>"))
                }
            }

            info!("remote {:?} -> request for {:?}", param!("REMOTE_ADDR"), param!("REQUEST_URI"));

            Ok(FastcgiRequest::new(
                begin_request.role,
                params,
                body_record_stream,
                id,
                response_sender,
            ))
        });

        let handler = self.handler.clone();
        let response_future = request_future
            .and_then(move |request| {
                // We need to start the handler and run it to completion.
                // Meanwhile, we need to get the first record from `response_receiver` and
                //  return it as a `Message` with a body,
                //  and continue to pump records out and send them to the body.

                // This makes a stream that yields nothing and finishes only once the handler is
                // done. It allows us to drive the handler while simultaneously pumping messages,
                // by merging the two streams together.
                let handler_stream: Box<Stream<Item = Option<FastcgiRecord>, Error = io::Error>>
                    = Box::new(
                        handler.call(request)
                            .into_stream()
                            .filter(|_| false)
                            .map(|_| None)
                            );

                // We also have `response_receiver`, which is a stream of `FastcgiRecord`,
                // which are the records the handler generates as it runs.
                let record_stream: Box<Stream<Item = Option<FastcgiRecord>, Error = io::Error>>
                    = Box::new(
                        response_receiver.map(Some)
                            .or_else(|()| Ok(None))
                    );

                // Take the first record received.
                handler_stream.select(record_stream)
                    .into_future()
                    .map_err(|(e, _stream)| e)
                    .map(move |(maybe_record, record_stream)| {
                        debug!("merged streams yielded something: {:?}", maybe_record);

                        match maybe_record {
                            Some(Some(record)) => {
                                debug!("first record received");

                                let end_records = vec![
                                    Ok(FastcgiRecord {
                                        request_id: id,
                                        body: FastcgiRecordBody::Stdout(EasyBuf::new()),
                                    }),
                                    Ok(FastcgiRecord {
                                        request_id: id,
                                        body: FastcgiRecordBody::Stderr(EasyBuf::new()),
                                    }),
                                    Ok(FastcgiRecord {
                                        request_id: id,
                                        body: FastcgiRecordBody::EndRequest(EndRequest {
                                            app_status: 0,
                                            protocol_status: ProtocolStatus::RequestComplete,
                                        })
                                    }),
                                ];

                                let records = record_stream
                                    .map(|maybe_record| maybe_record.unwrap())
                                    .chain(stream::iter(end_records))
                                    .then(Ok);

                                let (body_sender, body) = Body::<FastcgiRecord, io::Error>::pair();

                                // Schedule a future to pump remaining records through to the body.
                                // TODO: what if `handle()` returns `None`?
                                reactor_handle.handle().unwrap().spawn(
                                    body_sender.send_all(records)
                                        .then(|_| {
                                            debug!("done sending response body records");
                                            future::ok(())
                                        }));

                                // Start the response!
                                Message::WithBody(record, body)
                            },
                            None => {
                                debug!("no record received");
                                Message::WithoutBody(
                                    FastcgiRecord {
                                        request_id: id,
                                        body: FastcgiRecordBody::Stdout(EasyBuf::new())
                                    })
                            },
                            Some(None) => panic!("this is never supposed to happen")
                        }
                    })
            });

        Box::new(response_future)
    }
}
