use super::endian::*;
use super::rawstruct::*;
use super::s11n::*;

use byteorder::{ByteOrder, NetworkEndian};
use enum_primitive::FromPrimitive;
use tokio_core::io::{Codec, EasyBuf};

use std::io;
use std::mem::size_of;

#[derive(Debug)]
pub struct FastcgiRecord {
    pub record_type: RecordType,
    pub request_id: u16,
    pub content: FastcgiRecordBody,
    pub content_len: usize,
}

#[derive(Debug)]
pub enum FastcgiRecordBody {
    Data(EasyBuf),
    Params(Vec<(EasyBuf, EasyBuf)>),
    BeginRequest(BeginRequest),
    EndRequest(EndRequest),
}

impl FastcgiRecordBody {
    pub fn unwrap_begin_request(&self) -> &BeginRequest {
        match self {
            &FastcgiRecordBody::BeginRequest(ref x) => x,
            _ => panic!()
        }
    }

    pub fn unwrap_params(&self) -> &[(EasyBuf, EasyBuf)] {
        match self {
            &FastcgiRecordBody::Params(ref x) => x.as_slice(),
            _ => panic!()
        }
    }
}

#[derive(Debug)]
pub struct BeginRequest {
    pub role: Role,
    pub keep_connection: bool,
}

#[derive(Debug)]
pub struct EndRequest {
    pub app_status: u32,
    pub protocol_status: ProtocolStatus,
}

pub struct FastcgiLowlevelCodec;

fn read_header(buf: &mut EasyBuf) -> Option<FastcgiRecordHeader> {
    let header_len = size_of::<FastcgiRecordHeader>();
    if buf.len() < header_len {
        debug!("insufficient buffer for header");
        None
    } else {
        Some(unsafe { from_bytes(buf.drain_to(header_len).as_slice()) })
    }
}

fn read_len(buf: &mut EasyBuf) -> usize {
    let first_byte = buf.as_slice()[0];
    if first_byte < 0x80 {
        buf.drain_to(1).as_slice()[0] as usize
    } else {
        NetworkEndian::read_u32(buf.drain_to(4).as_slice()) as usize
    }
}

fn read_params(buf: &mut EasyBuf) -> Vec<(EasyBuf, EasyBuf)> {
    let mut params = vec![];
    loop {
        if buf.len() == 0 {
            break;
        }
        let name_len = read_len(buf);
        let value_len = read_len(buf);
        let name = buf.drain_to(name_len);
        let value = buf.drain_to(value_len);
        params.push((name, value));
    }
    params
}

fn read_begin_request_body(buf: &mut EasyBuf) -> io::Result<BeginRequest> {
    let len = buf.len();
    let raw: BeginRequestBody = unsafe { from_bytes(buf.drain_to(len).as_slice()) };
    let role = match Role::from_u16(raw.role.get()) {
        Some(role) => role,
        None => {
            let msg = format!("unknown role {}", raw.role.get());
            error!("{}", msg);
            return Err(io::Error::new(io::ErrorKind::InvalidData, msg))
        }
    };
    Ok(BeginRequest {
        role: role,
        keep_connection: (raw.flags & 1) == 1,
    })
}

impl Codec for FastcgiLowlevelCodec {
    type In = FastcgiRecord;
    type Out = FastcgiRecord;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Self::In>, io::Error> {
        debug!("buffer: {} bytes", buf.len());

        let header = match read_header(buf) {
            Some(header) => header,
            None => return Ok(None),
        };

        if header.version != FASTCGI_VERSION {
            let msg = format!("unexpected FCGI version {}", header.version);
            error!("{}", msg);
            return Err(io::Error::new(io::ErrorKind::InvalidData, msg));
        }

        let content_len = header.content_length.get() as usize;
        if buf.len() < content_len {
            debug!("insufficient buffer for the content");
            return Ok(None);
        }

        let record_type = RecordType::from_u8(header.record_type).unwrap_or_else(|| {
            warn!("unknwon record type {}", header.record_type);
            RecordType::UnknownType
        });
        let request_id = header.request_id.get();

        debug!("request id: {}; record type: {:?}, {} bytes of content",
               request_id, record_type, content_len);

        //let mut content = Vec::with_capacity(content_len);
        //content.extend_from_slice(buf.drain_to(content_len).as_slice());

        let content = match record_type {
            RecordType::BeginRequest => {
                FastcgiRecordBody::BeginRequest(read_begin_request_body(buf)?)
            },
            RecordType::Params => {
                FastcgiRecordBody::Params(read_params(buf))
            },
            _ => {
                FastcgiRecordBody::Data(buf.drain_to(content_len))
            }
        };

        if buf.len() < header.padding_length as usize {
            debug!("insufficient buffer for the padding");
            return Ok(None);
        }
        buf.drain_to(header.padding_length as usize);

        let message = FastcgiRecord {
            record_type: record_type,
            request_id: request_id,
            content: content,
            content_len: content_len,
        };

        Ok(Some(message))
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
        let data = match msg.content {
            FastcgiRecordBody::Data(buf) => buf,
            _ => panic!("only FastcgiRecordBody::Data is supported in FastcgiLowlevelCodec::Encode"),
        };

        let header = FastcgiRecordHeader {
            version: FASTCGI_VERSION,
            record_type: msg.record_type as u8,
            request_id: NetworkU16::new(msg.request_id),
            content_length: NetworkU16::new(data.len() as u16),
            padding_length: 0,
            reserved: 0,
        };
        buf.extend_from_slice(unsafe { as_bytes(&header) });
        buf.extend_from_slice(data.as_slice());

        Ok(())
    }
}
