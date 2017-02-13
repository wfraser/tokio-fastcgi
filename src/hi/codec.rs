use super::super::*;

use tokio_core::io::{Codec, EasyBuf};
use tokio_proto::streaming::multiplex::*;

use std::io;

pub struct FastcgiMultiplexedPipelinedCodec {
    inner: FastcgiLowlevelCodec,
}

impl FastcgiMultiplexedPipelinedCodec {
    pub fn new() -> FastcgiMultiplexedPipelinedCodec {
        FastcgiMultiplexedPipelinedCodec {
            inner: FastcgiLowlevelCodec,
        }
    }
}

impl Codec for FastcgiMultiplexedPipelinedCodec {
    type In = Frame<FastcgiRecord, FastcgiRecord, io::Error>;
    type Out = Frame<FastcgiRecord, FastcgiRecord, io::Error>;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Self::In>, io::Error> {
        match self.inner.decode(buf) {
            Ok(Some(record)) => {
                match record.body {
                    FastcgiRecordBody::BeginRequest(_) => {
                        debug!("got BeginRequest, sending header chunk");
                        Ok(Some(Frame::Message {
                            id: record.request_id as RequestId,
                            message: record,
                            body: true,
                            solo: false
                        }))
                    },
                    FastcgiRecordBody::Stdin(ref buf) if buf.len() == 0 => {
                        debug!("stdin is done; sending empty body chunk");
                        Ok(Some(Frame::Body {
                            id: record.request_id as RequestId,
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
                            id: record.request_id as RequestId,
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

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
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
