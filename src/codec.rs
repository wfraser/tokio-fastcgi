use super::byteorder_ext::*;
use super::rawstruct::*;

use enum_primitive::FromPrimitive;
use tokio_core::io::{Codec, EasyBuf};
use tokio_proto::streaming::pipeline::Frame;

use std::io;
use std::mem::size_of;

pub const FASTCGI_VERSION: u8 = 1;

enum_from_primitive! {
    #[repr(u8)]
    #[derive(Debug, PartialEq)]
    pub enum RecordType {
        BeginRequest = 1,
        AbortRequest = 2,
        EndRequest = 3,
        Params = 4,
        Stdin = 5,
        Stdout = 6,
        Stderr = 7,
        Data = 8,
        GetValues = 9,
        GetValuesResult = 10,
        UnknownType = 11,
        Mystery,
    }
}

fn is_stream(record_type: RecordType) -> bool {
    match record_type {
        RecordType::Params | RecordType::Stdin | RecordType::Stdout | RecordType::Stderr
            | RecordType::Data => true,
        _ => false
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
struct FastcgiRecordHeader {
    version: u8,
    record_type: u8,
    request_id: u16,
    content_length: u16,
    padding_length: u8,
    reserved: u8,
}

#[derive(Debug)]
pub struct FastcgiRecord {
    pub record_type: RecordType,
    pub request_id: u16,
    pub content: Vec<u8>,
}

pub struct FastcgiCodec;

impl Codec for FastcgiCodec {
    type In = Frame<FastcgiRecord, FastcgiRecord, io::Error>;
    type Out = Frame<FastcgiRecord, FastcgiRecord, io::Error>;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Self::In>, io::Error> {
        let header_len = size_of::<FastcgiRecordHeader>();
        if buf.len() < header_len {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof,
                "not enough bytes for header"));
        }
        let header_bytes = buf.drain_to(header_len);
        let header: FastcgiRecordHeader = unsafe { from_bytes(header_bytes.as_slice()) };

        if header.version != FASTCGI_VERSION {
            let msg = format!("unexpected FCGI version {}", header.version);
            error!("{}", msg);
            return Err(io::Error::new(io::ErrorKind::InvalidData, msg));
        }

        let content_length = ntohs(header.content_length) as usize;
        let mut content = Vec::with_capacity(content_length);
        content.extend_from_slice(buf.drain_to(content_length).as_slice());

        buf.drain_to(header.padding_length as usize);

        let record_type = RecordType::from_u8(header.record_type).unwrap_or_else(|| {
            warn!("unknown record type {}", header.record_type);
            RecordType::UnknownType
        });

        Ok(Some(Frame::Message {
            message: FastcgiRecord {
                record_type: record_type,
                request_id: ntohs(header.request_id),
                content: content,
            },
            body: false,    // TODO: also support streams
        }))
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
        let mut record = match msg {
            Frame::Message { message, body: _ } => message,
            Frame::Body { chunk: message } => match message {
                Some(message) => message,
                None => {
                    let msg = "stream message not present";
                    error!("{}", msg);
                    return Err(io::Error::new(io::ErrorKind::InvalidData, msg));
                }
            },
            Frame::Error { error } => {
                return Err(error);
            }
        };

        let header = FastcgiRecordHeader {
            version: FASTCGI_VERSION,
            record_type: record.record_type as u8,
            request_id: record.request_id,
            content_length: htons(record.content.len() as u16),
            padding_length: 0,
            reserved: 0,
        };
        buf.extend_from_slice(unsafe { as_bytes(&header) });
        buf.append(&mut record.content);

        Ok(())
    }
}
