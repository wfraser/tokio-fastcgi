use super::super::*;

use futures::{Stream, Sink, Poll, StartSend};
use tokio_core::io::{Framed, Io};
use tokio_proto::streaming::multiplex::*;

use std::io;

pub struct FastcgiTransport<IO: Io + 'static> {
    inner: Framed<IO, FastcgiMultiplexedPipelinedCodec>,
}

impl<IO: Io + 'static> FastcgiTransport<IO> {
    pub fn new(io: IO) -> FastcgiTransport<IO> {
        let codec = FastcgiMultiplexedPipelinedCodec::new();
        FastcgiTransport {
            inner: io.framed(codec),
        }
    }
}

impl<IO: Io + 'static, ReadBody> Transport<ReadBody> for FastcgiTransport<IO> {
    // TODO: this is a nice place to put some extra logic. See:
    // https://tokio-rs.github.io/tokio-proto/tokio_proto/streaming/multiplex/trait.Transport.html
}

impl<IO: Io + 'static> Stream for FastcgiTransport<IO> {
    type Item = Frame<FastcgiRecord, FastcgiRecord, io::Error>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.inner.poll()
    }
}

impl<IO: Io + 'static> Sink for FastcgiTransport<IO> {
    type SinkItem = Frame<FastcgiRecord, FastcgiRecord, io::Error>;
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        self.inner.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.inner.poll_complete()
    }
}
