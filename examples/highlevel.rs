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
    pub fn new(role: Role) -> FastcgiRequestState {
        FastcgiRequestState {
            role: role,
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
                if rec.record_type != RecordType::BeginRequest {
                    let msg = format!("first request with id {} was {:?}, not BeginRequest",
                                      rec.request_id, rec.record_type);
                    error!("{}", msg);
                    return future::err(io::Error::new(io::ErrorKind::InvalidData, msg)).boxed();
                } else {
                    let role = rec.content.unwrap_begin_request().role;
                    entry.insert(FastcgiRequestState::new(role));
                    return future::ok(()).boxed();
                }
            },
            Entry::Occupied(mut entry) => {
                let state = entry.get_mut();
                match rec.record_type {
                    RecordType::GetValuesResult | RecordType::EndRequest | RecordType::Stdout
                            | RecordType::Stderr => {
                        let msg = format!("unexpected record type {:?} from web server",
                                          rec.record_type);
                        return future::err(io::Error::new(io::ErrorKind::InvalidData, msg)).boxed();
                    },
                    RecordType::Params => {
                        if rec.content_len == 0 {
                            // Issue request now
                            debug!("issue request now");
                            state.headers_done = true;
                        } else {
                            for &(ref name, ref value) in rec.content.unwrap_params() {
                                state.set_param(Vec::from(name.as_slice()), value.clone());
                            }
                        }
                    },
                    _ => unimplemented!()
                }
            }
        };

        // TODO
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
