use super::super::*;

use tokio_core::io::Io;
use tokio_proto::streaming::multiplex::*;

use std::io;

#[derive(Debug, Default)]
pub struct FastcgiProto;

impl<IO: Io + 'static> ServerProto<IO> for FastcgiProto {
    type Request = FastcgiRecord;
    type RequestBody = FastcgiRecord;
    type Response = FastcgiRecord;
    type ResponseBody = FastcgiRecord;
    type Error = io::Error;

    type Transport = FastcgiTransport<IO>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: IO) -> Self::BindTransport {
        Ok(FastcgiTransport::new(io))
    }
}
