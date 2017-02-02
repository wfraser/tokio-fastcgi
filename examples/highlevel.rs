extern crate tokio_fastcgi;
use tokio_fastcgi::*;

extern crate env_logger;
extern crate futures;
#[macro_use] extern crate log;
extern crate tokio_core;
extern crate tokio_uds;

use futures::{future, Future, Stream};
use tokio_core::reactor::Core;
use tokio_core::io::{Io, EasyBuf};
use tokio_uds::*;

use std::collections::btree_map::*;
use std::collections::HashMap;
use std::fs;
use std::io;

fn umask(mask: u32) -> u32 {
    extern "system" { fn umask(mask: u32) -> u32; }
    unsafe { umask(mask) }
}

// TODO: experimental code; most of the following should go in the main crate once it works.

struct FastcgiRequestState {
    pub role: Role,
    pub params: HashMap<Vec<u8>, EasyBuf>,
    pub headers_done: bool,
}

impl FastcgiRequestState {
    pub fn new(details: &BeginRequest) -> FastcgiRequestState {
        FastcgiRequestState {
            role: details.role,
            params: HashMap::new(),
            headers_done: false,
        }
    }

    // Sadly, EasyBuf doesn't implement Hash or Eq, so we have to copy it to a Vec to use it as the
    // key of a HashMap. Luckily, header and environment variable names are usually very short.
    pub fn set_param(&mut self, key: Vec<u8>, value: EasyBuf) {
        self.params.insert(key, value);
    }
}

struct FastcgiMultiplexer {
    requests: BTreeMap<u16, FastcgiRequestState>,
}

impl FastcgiMultiplexer {
    pub fn new() -> FastcgiMultiplexer {
        FastcgiMultiplexer {
            requests: BTreeMap::new(),
        }
    }

    pub fn process_record(&mut self, rec: FastcgiRecord) -> Box<Future<Item = (), Error = io::Error>> {
        match self.requests.entry(rec.request_id) {
            Entry::Vacant(entry) => {
                match rec.body {
                    FastcgiRecordBody::BeginRequest(ref details) => {
                        entry.insert(FastcgiRequestState::new(details));
                        return future::ok(()).boxed();
                    },
                    _ => {
                        let msg = format!("first request with id {} was {:?}, not BeginRequest",
                                          rec.request_id, rec.body);
                        error!("{}", msg);
                        return future::err(io::Error::new(io::ErrorKind::InvalidData, msg)).boxed();
                    }
                }
            },
            Entry::Occupied(mut entry) => {
                let state = entry.get_mut();
                match rec.body {
                    FastcgiRecordBody::Params(ref params) => {
                        if params.len() == 0 {
                            debug!("issue request now");
                            state.headers_done = true;
                        } else {
                            for &(ref name, ref value) in params {
                                state.set_param(Vec::from(name.as_slice()), value.clone());
                            }
                        }
                    },
                    // TODO
                    _ => panic!("support for {:?} not implemented yet", rec.body),
                }
            }
        };

        future::ok(()).boxed()
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

    umask(0);
    let listener = UnixListener::bind(filename, &reactor.handle()).expect("failed to bind socket");

    let srv = listener.incoming().for_each(move |(socket, _addr)| {
        println!("{:#?}", socket);

        let mut mux = FastcgiMultiplexer::new();

        socket.framed(FastcgiLowlevelCodec)
            .for_each(move |record| {
                mux.process_record(record)
            })
    });

    reactor.run(srv).expect("failed to run the server");
}
