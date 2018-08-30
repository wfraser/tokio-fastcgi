use super::super::*;

use bytes::BytesMut;
use tokio_io::codec::{Decoder, Encoder};
use tokio_proto::streaming::multiplex::*;

use std::io;

#[derive(Debug, Default)]
pub struct FastcgiMultiplexedPipelinedCodec {
    inner: FastcgiLowlevelCodec,
}

impl Decoder for FastcgiMultiplexedPipelinedCodec {
    type Item = Frame<FastcgiRecord, FastcgiRecord, io::Error>;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match self.inner.decode(buf) {
            Ok(Some(record)) => {
                match record.body {
                    FastcgiRecordBody::BeginRequest(_) => {
                        debug!("got BeginRequest, sending header chunk");
                        Ok(Some(Frame::Message {
                            id: RequestId::from(record.request_id),
                            message: record,
                            body: true,
                            solo: false
                        }))
                    },
                    FastcgiRecordBody::Stdin(ref buf) if buf.is_empty() => {
                        debug!("stdin is done; sending empty body chunk");
                        Ok(Some(Frame::Body {
                            id: RequestId::from(record.request_id),
                            chunk: None,
                        }))
                    },
                    _ => {
                        if let FastcgiRecordBody::Data(_) = record.body {
                            // This is only used by the "Filter" role.
                            warn!("FCGI_DATA not supported");
                        }
                        debug!("sending body chunk");
                        Ok(Some(Frame::Body {
                            id: RequestId::from(record.request_id),
                            chunk: Some(record)
                        }))
                    }
                }
            },
            Ok(None) => {
                debug!("got None from underlying codec");
                Ok(None)
            },
            Err(e) => {
                error!("got error from underlying codec: {}", e);
                Err(e)
            }
        }
    }
}

impl Encoder for FastcgiMultiplexedPipelinedCodec {
    type Item = Frame<FastcgiRecord, FastcgiRecord, io::Error>;
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
        match msg {
            Frame::Message { id, message, body, solo } => {
                debug!("encoding message: {} {:?} body={:?} solo={:?}", id, message, body, solo);
                self.inner.encode(message, buf)
            },
            Frame::Body { id, chunk } => {
                debug!("encoding body: {} {:?}", id, chunk);
                if let Some(rec) = chunk {
                    self.inner.encode(rec, buf)
                } else {
                    Ok(())
                }
            },
            Frame::Error { id, error } => {
                error!("error: {} {}", id, error);
                Err(error)
            }
        }
    }
}
