extern crate tokio_fastcgi;
use tokio_fastcgi::*;

extern crate bytes;
extern crate env_logger;
extern crate futures;
extern crate tokio_codec;
extern crate tokio_core;
extern crate tokio_uds;

use bytes::BytesMut;
use futures::{future, Stream};
use tokio_codec::Decoder;
use tokio_core::reactor::Core;
use tokio_uds::*;

use std::fs;
use std::io;

fn umask(mask: u32) -> u32 {
    extern "system" { fn umask(mask: u32) -> u32; }
    unsafe { umask(mask) }
}

fn print_data(buf: &BytesMut) {
    for (i, byte) in buf.iter().enumerate() {
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
}

fn print_record(record: &FastcgiRecord) {
    println!("------------------------------------------------------------------------------");
    println!("request id: {}", record.request_id);
    match record.body {
        FastcgiRecordBody::Params(ref params) => {
            println!("params - {}:", params.len());
            for &(ref name, ref value) in params {
                println!("  {} = {}",
                         String::from_utf8_lossy(name),
                         String::from_utf8_lossy(value));
            }
        },
        FastcgiRecordBody::Data(ref buf) => {
            println!("Data - {} bytes:", buf.len());
            print_data(buf);
        },
        FastcgiRecordBody::Stdin(ref buf) => {
            println!("Stdin - {} bytes", buf.len());
            print_data(buf);
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

        FastcgiLowlevelCodec.framed(socket)
            .take_while(move |record| {
                print_record(record);
                let done = match record.body {
                    FastcgiRecordBody::Stdin(ref buf) if buf.is_empty() => true,
                    _ => false
                };
                future::ok(!done)
            })
            .for_each(|_| future::ok(()))
    });

    reactor.run(srv).expect("failed to run the server");
}
