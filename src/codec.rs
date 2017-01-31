use super::byteorder_ext::*;
use super::rawstruct::*;

use enum_primitive::FromPrimitive;
use tokio_core::io::{Codec, EasyBuf};
use tokio_proto::streaming::multiplex::{Frame, RequestId};

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
        debug!("buffer: {} bytes", buf.len());

        let header_len = size_of::<FastcgiRecordHeader>();
        if buf.len() < header_len {
            debug!("insufficient buffer for the header");
            return Ok(None);
        }
        let header_bytes = buf.drain_to(header_len);
        let header: FastcgiRecordHeader = unsafe { from_bytes(header_bytes.as_slice()) };

        if header.version != FASTCGI_VERSION {
            let msg = format!("unexpected FCGI version {}", header.version);
            error!("{}", msg);
            return Err(io::Error::new(io::ErrorKind::InvalidData, msg));
        }

        let content_len = ntohs(header.content_length) as usize;
        if buf.len() < content_len {
            debug!("insufficient buffer for the content");
            return Ok(None);
        }
        let mut content = Vec::with_capacity(content_len);
        content.extend_from_slice(buf.drain_to(content_len).as_slice());

        if buf.len() < header.padding_length as usize {
            debug!("insufficient buffer for the padding");
            return Ok(None);
        }
        buf.drain_to(header.padding_length as usize);

        let record_type = RecordType::from_u8(header.record_type).unwrap_or_else(|| {
            warn!("unknown record type {}", header.record_type);
            RecordType::UnknownType
        });
        let request_id = ntohs(header.request_id);

        debug!("request id: {}; record type: {:?}", request_id, record_type);

        let message = FastcgiRecord {
            record_type: record_type,
            request_id: request_id,
            content: content,
        };

        let frame = match message.record_type {
            RecordType::BeginRequest => Frame::Message {
                id: request_id as RequestId,
                message: message,
                body: true,
                solo: false,
            },
            RecordType::AbortRequest
                    | RecordType::Stdin if message.content.is_empty()
                    => {
                debug!("got empty stdin chunk");
                Frame::Body {
                    id: request_id as RequestId,
                    chunk: None,
                }
            },
            RecordType::GetValuesResult
                    | RecordType::EndRequest
                    | RecordType::Stdout
                    | RecordType::Stderr
                    => {
                let msg = format!("unexpected message type from web server: {:?}",
                                  message.record_type);
                error!("{}", msg);
                Frame::Error {
                    id: request_id as RequestId,
                    error: io::Error::new(io::ErrorKind::InvalidData, msg),
                }
            },
            _ => Frame::Body {
                id: request_id as RequestId,
                chunk: Some(message),
            }
        };

        Ok(Some(frame))
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
        let mut record = match msg {
            Frame::Message { id: _, message, body: _, solo: _ } => message,
            Frame::Body { id: _, chunk: message } => match message {
                Some(message) => message,
                None => {
                    let msg = "stream message not present";
                    error!("{}", msg);
                    return Err(io::Error::new(io::ErrorKind::InvalidData, msg));
                }
            },
            Frame::Error { id: _, error } => {
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
