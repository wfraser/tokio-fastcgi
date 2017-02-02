extern crate tokio_fastcgi;
use tokio_fastcgi::*;

extern crate env_logger;
extern crate futures;
extern crate tokio_core;
extern crate tokio_uds;

use futures::{future, Stream};
use tokio_core::reactor::Core;
use tokio_core::io::Io;
use tokio_uds::*;

use std::fs;
use std::io;

fn umask(mask: u32) -> u32 {
    extern "system" { fn umask(mask: u32) -> u32; }
    unsafe { umask(mask) }
}

fn print_record(record: &FastcgiRecord) {
    println!("request id: {}", record.request_id);
    match record.body {
        FastcgiRecordBody::Params(ref params) => {
            for &(ref name, ref value) in params {
                println!("param: {} = {}",
                         String::from_utf8_lossy(name.as_slice()),
                         String::from_utf8_lossy(value.as_slice()));
            }
        },
        FastcgiRecordBody::Data(ref buf) => {
            for (i, byte) in buf.as_slice().iter().enumerate() {
                if i % 8 == 0 {
                    if i != 0 {
                        println!("");
                    }
                    print!("\t");
                } else if i % 4 == 0 {
                    print!(" ");
                }
                print!("{:02X} ", byte);
            }
        },
        ref content => {
            println!("body: {:#?}", content);
        }
    }
}

fn main() {
    env_logger::init().unwrap();

    let filename = "hello.sock";

    let mut reactor = Core::new().expect("failed to create reactor core");

    if let Err(e) = fs::remove_file(filename) {
        if e.kind() != io::ErrorKind::NotFound {
            panic!("failed to remove existing socket file {:?}: {}", filename, e);
        }
    }

    umask(0);
    let listener = UnixListener::bind(filename, &reactor.handle()).expect("failed to bind socket");

    let srv = listener.incoming().for_each(move |(socket, _addr)| {
        println!("{:#?}", socket);

        socket.framed(FastcgiLowlevelCodec)
            .take_while(move |record| {
                print_record(record);
                let done = match record.body {
                    FastcgiRecordBody::Stdin(ref buf) if buf.len() == 0 => true,
                    _ => false
                };
                future::ok(!done)
            })
            .for_each(|_| future::ok(()))
    });

    reactor.run(srv).expect("failed to run the server");
}
