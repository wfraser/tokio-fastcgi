use super::codec::*;

use tokio_core::io::{Io, Framed};
use tokio_proto::streaming::multiplex::ServerProto;

use std::io;

pub struct FastcgiProto;

impl<T: Io + 'static> ServerProto<T> for FastcgiProto {
    type Request = FastcgiRecord;
    type RequestBody = FastcgiRecord;
    type Response = FastcgiRecord;
    type ResponseBody = FastcgiRecord;
    type Error = io::Error;

    type Transport = Framed<T, FastcgiCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(FastcgiCodec))
    }
}
