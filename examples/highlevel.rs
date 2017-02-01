extern crate tokio_fastcgi;
use tokio_fastcgi::*;

extern crate byteorder;
extern crate enum_primitive;
extern crate env_logger;
extern crate futures;
#[macro_use] extern crate log;
extern crate tokio_core;
extern crate tokio_uds;

use byteorder::{ByteOrder, NetworkEndian};
use enum_primitive::FromPrimitive;
use futures::{future, Future, Stream};
use tokio_core::reactor::Core;
use tokio_core::io::Io;
use tokio_uds::*;

use std::collections::btree_map::*;
use std::collections::HashMap;
use std::fs;
use std::io;

// TODO: experimental code; most of the following should go in the main crate once it works.

struct FastcgiRequestState {
    pub role: Role,
    pub params: HashMap<Vec<u8>, Vec<u8>>,
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

    pub fn set_param(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.params.insert(key, value);
    }
}

struct FastcgiMultiplexer {
    requests: BTreeMap<u16, FastcgiRequestState>,
}

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

fn read_params(state: &mut FastcgiRequestState, buf: &[u8]) {
    let mut idx = 0;
    loop {
        if idx == buf.len() {
            break;
        }
        let name_len = read_len(&mut idx, buf);
        let value_len = read_len(&mut idx, buf);
        let mut name = Vec::with_capacity(name_len);
        let mut value = Vec::with_capacity(value_len);
        name.extend_from_slice(&buf[idx .. idx + name_len]);
        idx += name_len;
        value.extend_from_slice(&buf[idx .. idx + value_len]);
        idx += value_len;
        state.set_param(name, value);
    }
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
                    let body: BeginRequestBody = unsafe { from_bytes(&rec.content) };
                    let role = match Role::from_u16(body.role.get()) {
                        Some(role) => role,
                        None => {
                            let msg = format!("unknown role {}", body.role.get());
                            error!("{}", msg);
                            return future::err(io::Error::new(io::ErrorKind::InvalidData, msg)).boxed();
                        }
                    };
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
                        if rec.content.len() == 0 {
                            // Issue request now
                            debug!("issue request now");
                            state.headers_done = true;
                        } else {
                            read_params(state, &rec.content);
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
