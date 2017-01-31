extern crate byteorder;
#[macro_use] extern crate enum_primitive;
extern crate futures;
#[macro_use] extern crate log;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_uds;

mod endian;
mod codec;
mod proto;
mod rawstruct;
mod s11n;

pub use codec::{FastcgiCodec, FastcgiRecord};
pub use proto::FastcgiProto;
pub use rawstruct::*; // TODO: shouldn't expose this really
pub use s11n::{RecordType, FASTCGI_VERSION, BeginRequestBody};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
