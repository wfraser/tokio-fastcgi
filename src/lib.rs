#[macro_use] extern crate enum_primitive;
#[macro_use] extern crate log;

mod endian;
mod hi;
mod lowlevel;
mod rawstruct;
mod s11n;

/*
pub use hi::codec::FastcgiMultiplexedPipelinedCodec;
pub use hi::handler::FastcgiRequestHandler;
pub use hi::proto::FastcgiProto;
pub use hi::response::{FastcgiRequest, FastcgiHeadersResponse, FastcgiBodyResponse};
pub use hi::service::FastcgiService;
pub use hi::stream_process::StreamProcess;
pub use hi::transport::FastcgiTransport;
*/
pub use lowlevel::{FastcgiLowlevelCodec, FastcgiRecord, FastcgiRecordBody, BeginRequest, EndRequest};
pub use s11n::{FASTCGI_VERSION, Role, ProtocolStatus};
