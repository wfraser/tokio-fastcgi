use super::super::*;

use futures::{Stream, Sink, Poll, StartSend, Async};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_codec::{Decoder, Framed};
use tokio_proto::streaming::multiplex::*;

use std::collections::BTreeSet;
use std::io;

/// Manages a FastCGI connection, by binding a codec that translates bytes into multiplexed FastCGI
/// streams. This also closes the connection when no streams are active (unless one of them
/// specifies the `FCGI_KEEP_CONN` bit in the `BeginRequest` record).
pub struct FastcgiTransport<IO: AsyncRead + AsyncWrite + 'static> {
    inner: Option<Framed<IO, FastcgiMultiplexedPipelinedCodec>>,
    in_flight: Requests,
    keep_connection: bool,
}

// We want to drop connections only if no requests are in flight and we've seen at least one
// connection before. This is to guard against the case where the underlying transport returns
// NotReady before a full frame has been read, and poll_complete ends up being called with no
// requests seen yet.
struct Requests {
    in_flight: BTreeSet<u16>,
    any_yet: bool,
}

impl Requests {
    fn new() -> Requests {
        Requests {
            in_flight: BTreeSet::new(),
            any_yet: false,
        }
    }

    fn request(&mut self, id: RequestId) {
        self.in_flight.insert(id as u16);
        self.any_yet = true;
    }

    fn remove(&mut self, id: RequestId) {
        self.in_flight.remove(&(id as u16));
    }

    fn is_empty(&self) -> bool {
        self.any_yet && self.in_flight.is_empty()
    }
}

impl<IO: AsyncRead + AsyncWrite + 'static> FastcgiTransport<IO> {
    pub fn new(io: IO) -> FastcgiTransport<IO> {
        let codec = FastcgiMultiplexedPipelinedCodec::default();
        FastcgiTransport {
            inner: Some(codec.framed(io)),
            in_flight: Requests::new(),
            keep_connection: false,
        }
    }
}

impl<IO: AsyncRead + AsyncWrite + 'static, ReadBody> Transport<ReadBody> for FastcgiTransport<IO> {
    // This is a nice place to put some extra logic if needed. See:
    // https://tokio-rs.github.io/tokio-proto/tokio_proto/streaming/multiplex/trait.Transport.html
}

impl<IO: AsyncRead + AsyncWrite + 'static> Stream for FastcgiTransport<IO> {
    type Item = Frame<FastcgiRecord, FastcgiRecord, io::Error>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        trace!("poll");
        match self.inner.as_mut() {
            Some(io) => {
                debug!("poll: calling inner IO");
                let result = io.poll();
                debug!("poll: got {:?}", result);

                let id = match result {
                    Ok(Async::Ready(Some(ref frame))) => {
                        match *frame {
                            Frame::Message { id, ref message, .. } => {
                                if let FastcgiRecordBody::BeginRequest(ref begin_request) = message.body {
                                    if begin_request.keep_connection {
                                        debug!("request has FCGI_KEEP_CONN set");
                                        self.keep_connection = true;
                                    }
                                }
                                Some(id)
                            }
                            Frame::Body { id, chunk: Some(_) } => Some(id),
                            _ => None,
                        }
                    },
                    _ => None,
                };

                if let Some(id) = id {
                    debug!("poll: request ID is {}", id);
                    self.in_flight.request(id);
                }

                result
            },
            None => {
                debug!("poll: have no inner IO; returning Ready - None");
                Ok(Async::Ready(None))
            }
        }
    }
}

impl<IO: AsyncRead + AsyncWrite + 'static> Sink for FastcgiTransport<IO> {
    type SinkItem = Frame<FastcgiRecord, FastcgiRecord, io::Error>;
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        trace!("start_send: {:?}", item);
        if let Frame::Body { id, chunk: None } = item {
            debug!("start_send: request {} is finished", id);
            self.in_flight.remove(id);
        }

        self.inner.as_mut().expect("start_send called on dead connection").start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        trace!("poll_complete");
        let result = match self.inner.as_mut() {
            Some(inner) => inner.poll_complete(),
            None => {
                // poll_complete is basically a flush operation, so on a dead connection there's
                // nothing to do. Callers shouldn't do this, but if it happens, it's okay.
                info!("poll_complete called on a dead connection");
                Ok(Async::Ready(()))
            }
        };
        if !self.keep_connection && self.in_flight.is_empty() {
            debug!("poll_complete: zero in-flight requests; dropping connection.");
            self.inner = None;
        }
        result
    }
}
