use super::super::*;

use tokio_core::io::{Framed, Io};
use tokio_proto::streaming::multiplex::*;

use std::io;

pub struct FastcgiProto;

impl<IO: Io + 'static> ServerProto<IO> for FastcgiProto {
    type Request = FastcgiRecord;
    type RequestBody = FastcgiRecord;
    type Response = FastcgiRecord;
    type ResponseBody = FastcgiRecord;
    type Error = io::Error;

    type Transport = Framed<IO, FastcgiMultiplexedPipelinedCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: IO) -> Self::BindTransport {
        let codec = FastcgiMultiplexedPipelinedCodec::new();
        Ok(io.framed(codec))
    }
}
