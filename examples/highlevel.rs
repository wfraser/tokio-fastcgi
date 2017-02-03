extern crate tokio_fastcgi;
use tokio_fastcgi::*;

extern crate env_logger;
extern crate futures;
#[macro_use] extern crate log;
extern crate tokio_core;
extern crate tokio_uds;

use futures::{Future, Stream};
use tokio_core::reactor::Core;
use tokio_uds::*;

use std::fs;
use std::io;

fn umask(mask: u32) -> u32 {
    extern "system" { fn umask(mask: u32) -> u32; }
    unsafe { umask(mask) }
}

fn main() {
    env_logger::init().unwrap();

    let filename = "hello.sock";
    if let Err(e) = fs::remove_file(filename) {
        if e.kind() != io::ErrorKind::NotFound {
            panic!("failed to remove existing socket file {:?}: {}", filename, e);
        }
    }

    let mut reactor = Core::new().unwrap();

    umask(0);
    let listener = UnixListener::bind(filename, &reactor.handle()).expect("failed to bind socket");

    let srv = listener.incoming().for_each(move |(socket, _addr)| {
        println!("{:#?}", socket);

        FastcgiServer::new().bind(socket)
            .for_each(|request| {
                println!("YAY! Got a request for {:?}",
                         String::from_utf8_lossy(
                             request.params.get("REQUEST_URI")
                                .map(|x| x.as_slice())
                                .unwrap_or(b"<no REQUEST_URI set>")));
                Ok(())
            })
            .then(|_| {
                println!("socket closed");
                Ok(())
            })
    });

    reactor.run(srv).expect("failed to run the server");
}
