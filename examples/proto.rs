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
use tokio_core::io::EasyBuf;
use tokio_proto::BindServer;
use tokio_proto::streaming::{Message, Body};
use tokio_service::Service;
use tokio_uds::*;

use std::collections::HashMap;
use std::fs;
use std::io;

fn umask(mask: u32) -> u32 {
    extern "system" { fn umask(mask: u32) -> u32; }
    unsafe { umask(mask) }
}

//
// Experimental code that will all go in the main crate once it's working:
//

struct FastcgiService<F> {
    core_handle: Handle,
    handler: F,
}

impl<F> FastcgiService<F>
        where F: FnMut(HashMap<String, String>,
                       Box<Stream<Item = FastcgiRecord, Error = io::Error>>)
                -> Box<Future<Item = Message<FastcgiRecord, Body<FastcgiRecord, io::Error>>, Error = io::Error>>
{
    pub fn new(core_handle: Handle, handler: F) -> FastcgiService<F> {
        FastcgiService {
            core_handle: core_handle,
            handler: handler,
        }
    }
}

impl<F> Service for FastcgiService<F>
        where F: FnMut(HashMap<String, String>,
                       Box<Stream<Item = FastcgiRecord, Error = io::Error>>)
                -> Box<Future<Item = Message<FastcgiRecord, Body<FastcgiRecord, io::Error>>, Error = io::Error>>
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

        let handle = self.core_handle.clone();

        Box::new(stream_process.then(move |result| {
            let (body_record_stream, params) = result.unwrap();
            debug!("making the response: {:?}", params);

            // Doesn't work because lifetimes...
            //(self.handler)(params, body_record_stream.boxed())

            let data = format!(
                concat!(
                    "X-Powered-By: tokio-fastcgi/0.1\r\n",
                    "Content-Type: text/plain\r\n",
                    "\r\n",
                    "Hello from {:?}!\n"),
                params["REQUEST_URI"]);
            let out = EasyBuf::from(Vec::from(data.as_bytes()));

            let (body_sender, body) = Body::pair();

            let resp = Message::WithBody(
                FastcgiRecord {
                    request_id: id,
                    body: FastcgiRecordBody::Stdout(out),
                },
                body);

            handle.spawn_fn(move || {
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

                if !begin_request.keep_connection {
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

            debug!("sending response");
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

    umask(0);
    let listener = UnixListener::bind(filename, &reactor.handle()).expect("failed to bind socket");

    let srv = listener.incoming().for_each(move |(socket, _addr)| {
        println!("{:#?}", socket);

        let service = FastcgiService::new(handle.clone(), |_params, _body_record_stream| {
            // TODO: this currently isn't actually run because of lifetime problems noted above.
            future::ok(Message::WithoutBody(FastcgiRecord {
                request_id: 0,
                body: FastcgiRecordBody::Stdout(EasyBuf::new())
            })).boxed()
        });
        let proto = FastcgiProto;
        proto.bind_server(&handle, socket, service);

        Ok(())
    });

    reactor.run(srv).expect("failed to run the server");
}
