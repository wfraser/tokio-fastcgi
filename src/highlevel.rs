use super::lowlevel::FastcgiRecord;

use futures::{Future, Sink, Stream, Poll, StartSend, Async};
use tokio_core::io::Io;
use tokio_proto::multiplex::{RequestId, ServerProto};

use std::collections::HashMap;
use std::io;

pub struct FastcgiRequest {
    pub request_id: u16,
    pub params: HashMap<Vec<u8>, Vec<u8>>,
    //pub body: Box<Future<Item = Vec<u8>, Error = io::Error>>,
}

pub struct FastcgiResponse {
    pub request_id: u16,
    pub app_status: u32,
    pub protocol_status: u8,
    pub body: Vec<u8>,
    //pub body: Box<Future<Item = Vec<u8>, Error = io::Error>>,
}

pub struct FastcgiProto;

pub struct FastcgiTransport<T> {
    io: T,
}

impl<T> Stream for FastcgiTransport<T>
        where T: Stream<Item = FastcgiRecord, Error = io::Error>
               + Sink<SinkItem = FastcgiRecord, SinkError = io::Error>
{
    type Item = (RequestId, FastcgiRequest);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, io::Error> {
        unimplemented!()

        /*
        match self.io.poll() {
            Ok(Async::Ready(record)) => {
                println!("{:?}", record);
                unimplemented!()
            },
            Ok(Async::NotReady) => {
                return Ok(Async::NotReady);
            },
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    return Ok(Async::NotReady);
                } else {
                    return Err(e);
                }
            }
        }
        */
    }
}

impl<T> Sink for FastcgiTransport<T>
        where T: Stream<Item = FastcgiRecord, Error = io::Error>
               + Sink<SinkItem = FastcgiRecord, SinkError = io::Error>
{
    type SinkItem = (RequestId, FastcgiResponse);
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, io::Error> {
        /*
        let buf = [item as u8];

        match self.io.write(&buf) {
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    return Ok(AsyncSink::NotReady(item));
                } else {
                    return Err(e);
                }
            }
            Ok(n) => {
                assert_eq!(1, n);
                return Ok(AsyncSink::Ready);
            }
        }
        */
        unimplemented!()
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        self.io.poll_complete()
    }
}

impl<T> ServerProto<T> for FastcgiProto
        where T: Stream<Item = FastcgiRecord, Error = io::Error>
               + Sink<SinkItem = FastcgiRecord, SinkError = io::Error>
               + 'static
{
    type Request = FastcgiRequest;
    type Response = FastcgiResponse;
    type Transport = FastcgiTransport<T>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(FastcgiTransport {
            io: io
        })
    }
}
