use super::lowlevel::{self, FastcgiRecordBody};
use super::s11n::Role;

use futures::{future, Future, Sink, Stream};
use futures::sync::mpsc::{self, Sender, Receiver};
use tokio_core::io::{Io, EasyBuf};

use std::collections::btree_map::*;
use std::collections::HashMap;
use std::io;

pub struct FastcgiRequest {
    pub role: Role,
    pub params: HashMap<String, EasyBuf>,
    /*
    pub data: Receiver<EasyBuf>,
    pub stdin: Receiver<EasyBuf>,
    pub stdout: Sender<EasyBuf>,
    pub stderr: Sender<EasyBuf>,
    pub end_request: ???
    */
}

#[derive(Debug)]
struct RequestHeader {
    pub role: Role,
    pub params: HashMap<String, EasyBuf>,
}

struct RequestState {
    pending_header: Option<RequestHeader>,
    data: Option<Sender<EasyBuf>>,
    stdin: Option<Sender<EasyBuf>>,
}

impl RequestState {
    pub fn new(details: lowlevel::BeginRequest) -> RequestState {
        RequestState {
            pending_header: Some(RequestHeader {
                role: details.role,
                params: HashMap::new(),
            }),
            data: None,     // created upon sending the header
            stdin: None,    // created upon sending the header
        }
    }

    pub fn set_param(&mut self, key: EasyBuf, value: EasyBuf) {
        let key = String::from_utf8_lossy(key.as_slice()).into_owned();
        self.pending_header.as_mut().unwrap().params.insert(key, value);
    }
}

pub struct FastcgiServer {
    requests: BTreeMap<u16, RequestState>,
}

fn invalid_data(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

impl FastcgiServer {
    pub fn new() -> FastcgiServer {
        FastcgiServer {
            requests: BTreeMap::new(),
        }
    }

    pub fn bind<IO: Io + 'static>(mut self, io: IO)
            -> Box<Stream<Item = FastcgiRequest, Error = io::Error>> {
        let (mut record_sink, record_stream) = io.framed(lowlevel::FastcgiLowlevelCodec).split();
        let request_stream = record_stream
            .filter_map(move |record| {
                self.process_record(record, &mut record_sink)
            })
            .then(|request_result| {
                match request_result {
                    Ok(Ok(request)) => Ok(request),
                    Ok(Err(protocol_error)) => {
                        error!("protocol error: {}", protocol_error);
                        Err(protocol_error)
                    },
                    Err(io_error) => {
                        error!("I/O error: {}", io_error);
                        Err(io_error)
                    }
                }
            });
        Box::new(request_stream)
    }

    fn process_record<W>(&mut self, rec: lowlevel::FastcgiRecord, sink: &mut W)
            -> Option<Result<FastcgiRequest, io::Error>>
            where W: Sink<SinkItem = lowlevel::FastcgiRecord, SinkError = io::Error> {

        match self.requests.entry(rec.request_id) {
            Entry::Vacant(entry) => {
                match rec.body {
                    FastcgiRecordBody::BeginRequest(details) => {
                        entry.insert(RequestState::new(details));
                        return None;
                    },
                    _ => {
                        let msg = format!("first request with ID {} was {:?}, not BeginRequest",
                                          rec.request_id, rec.body);
                        error!("{}", msg);
                        return Some(Err(invalid_data(msg)))
                    }
                }
            },
            Entry::Occupied(mut entry) => {
                let state = entry.get_mut();
                match rec.body {
                    FastcgiRecordBody::Params(params) => {
                        if params.len() == 0 {
                            debug!("issue request now");


                            let header = state.pending_header.take().unwrap();
                            return Some(Ok(FastcgiRequest {
                                role: header.role,
                                params: header.params,
                                // TODO: set up channels for further communication
                            }))
                        } else {
                            for (name, value) in params.into_iter() {
                                state.set_param(name, value);
                            }
                        }
                    },
                    FastcgiRecordBody::BeginRequest(_) => {
                        let msg = format!("duplicate BeginRequest for ID {}", rec.request_id);
                        error!("{}", msg);
                        return Some(Err(invalid_data(msg)));
                    },
                    FastcgiRecordBody::Stdin(buf) => {
                        debug!("stdin of {} bytes", buf.len());
                        //TODO
                    },
                    FastcgiRecordBody::Data(buf) => {
                        debug!("data of {} bytes", buf.len());
                        //TODO
                    },
                    _ => panic!("support for {:?} not implemented or unexpected", rec.body),
                }
            },
        }

        None
    }
}
