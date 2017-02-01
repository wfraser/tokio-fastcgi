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

fn print_record(record: &FastcgiRecord) {
    println!("request id: {}", record.request_id);
    println!("request type: {:?}", record.record_type);
    println!("content length: {}", record.content_len);
    println!("body: {:#?}", record.content);
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

    let listener = UnixListener::bind(filename, &reactor.handle()).expect("failed to bind socket");

    let srv = listener.incoming().for_each(move |(socket, _addr)| {
        println!("{:#?}", socket);

        socket.framed(FastcgiLowlevelCodec)
            .take_while(move |record| {
                print_record(record);
                let done = record.record_type == RecordType::Stdin && record.content_len == 0;
                future::ok(!done)
            })
            .for_each(|_| future::ok(()))
    });

    reactor.run(srv).expect("failed to run the server");
}
