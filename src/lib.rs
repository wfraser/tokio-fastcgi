extern crate byteorder;
#[macro_use] extern crate enum_primitive;
extern crate futures;
#[macro_use] extern crate log;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_uds;

mod byteorder_ext;
mod codec;
mod proto;
mod rawstruct;

pub use codec::{FastcgiCodec, FastcgiRecord, RecordType, FASTCGI_VERSION};
pub use proto::FastcgiProto;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
