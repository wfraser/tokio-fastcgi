use crate::lowlevel::FastcgiRecord;

pub type RequestId = u64;

pub enum Frame {
    Message {
        id: RequestId,
        message: FastcgiRecord,
        body: bool,
        solo: bool,
    },
    Body {
        id: RequestId,
        chunk: Option<FastcgiRecord>,
    },
    Error {
        id: RequestId,
        error: std::io::Error,
    },
}
