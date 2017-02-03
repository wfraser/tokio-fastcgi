use super::endian::*;
use super::rawstruct::*;
use super::s11n::*;

use byteorder::{ByteOrder, NetworkEndian};
use enum_primitive::FromPrimitive;
use tokio_core::io::{Codec, EasyBuf, EasyBufMut};

use std::io;
use std::mem::size_of;

#[derive(Debug)]
pub struct FastcgiRecord {
    pub request_id: u16,
    pub body: FastcgiRecordBody,
}

#[derive(Debug)]
pub enum FastcgiRecordBody {
    BeginRequest(BeginRequest),
    AbortRequest,
    EndRequest(EndRequest),
    Params(Vec<(EasyBuf, EasyBuf)>),
    Stdin(EasyBuf),
    Stdout(EasyBuf),
    Stderr(EasyBuf),
    Data(EasyBuf),
    GetValues(Vec<EasyBuf>),
    GetValuesResult(Vec<(Vec<u8>, Vec<u8>)>),
    UnknownTypeResponse(u8),
    UnknownType(u8, EasyBuf), // this one is the the incoming record
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

fn write_len(buf: &mut EasyBufMut, len: usize) {
    if len < 0x80 {
        buf.push(len as u8);
    } else {
        let mut bytes = [0u8; 4];
        NetworkEndian::write_u32(bytes.as_mut(), len as u32);
        buf.extend_from_slice(bytes.as_ref());
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
        debug!("param ({}, {})",
               String::from_utf8_lossy(name.as_slice()),
               String::from_utf8_lossy(value.as_slice()));
        params.push((name, value));
    }
    params
}

fn write_params(params: Vec<(Vec<u8>, Vec<u8>)>) -> EasyBuf {
    let mut out = EasyBuf::new();
    {
        let mut out = out.get_mut();
        for (name, value) in params.into_iter() {
            write_len(&mut out, name.len());
            write_len(&mut out, value.len());
            out.extend_from_slice(name.as_slice());
            out.extend_from_slice(value.as_slice());
        }
    }
    out
}

fn read_begin_request_body(buf: &mut EasyBuf) -> io::Result<BeginRequest> {
    let len = size_of::<BeginRequestBody>();
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
            RecordType::Mystery
        });
        let request_id = header.request_id.get();

        debug!("request id: {}; record type: {:?}, {} bytes of content",
               request_id, record_type, content_len);

        let mut content_buf = buf.drain_to(content_len);
        let body = match record_type {
            RecordType::BeginRequest => {
                FastcgiRecordBody::BeginRequest(read_begin_request_body(&mut content_buf)?)
            },
            RecordType::AbortRequest => {
                assert_eq!(0, content_len);
                FastcgiRecordBody::AbortRequest
            },
            RecordType::Params => {
                FastcgiRecordBody::Params(read_params(&mut content_buf))
            },
            RecordType::Stdin => {
                FastcgiRecordBody::Stdin(content_buf)
            },
            RecordType::Data => {
                FastcgiRecordBody::Data(content_buf)
            },
            RecordType::GetValues => {
                let params = read_params(&mut content_buf);
                let names = params.into_iter().map(|(name, _value)| name).collect();
                FastcgiRecordBody::GetValues(names)
            },
            RecordType::Mystery => {
                FastcgiRecordBody::UnknownType(header.record_type, content_buf)
            },
            RecordType::EndRequest | RecordType::Stdout | RecordType::Stderr
                    | RecordType::GetValuesResult | RecordType::UnknownType => {
                let msg = format!("illegal record type {:?} from FastCGI client", record_type);
                error!("{}", msg);
                return Err(io::Error::new(io::ErrorKind::InvalidData, msg));
            },
        };

        if buf.len() < header.padding_length as usize {
            debug!("insufficient buffer for the padding");
            return Ok(None);
        }
        buf.drain_to(header.padding_length as usize);

        debug!("buffer now has {} bytes", buf.len());

        let message = FastcgiRecord {
            request_id: request_id,
            body: body,
        };

        Ok(Some(message))
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
        let (record_type, data): (RecordType, EasyBuf) = match msg.body {
            FastcgiRecordBody::Stdout(buf) => (RecordType::Stdout, buf),
            FastcgiRecordBody::Stderr(buf) => (RecordType::Stderr, buf),
            FastcgiRecordBody::EndRequest(end_body) => {
                let mut out = vec![0,0,0,0];
                NetworkEndian::write_u32(out.as_mut_slice(), end_body.app_status);
                out.push(end_body.protocol_status as u8);
                out.extend_from_slice(&[0,0,0]);
                (RecordType::EndRequest, EasyBuf::from(out))
            },
            FastcgiRecordBody::GetValuesResult(values) => {
                (RecordType::GetValuesResult, write_params(values))
            },
            FastcgiRecordBody::UnknownTypeResponse(typ) => {
                let out: Vec<u8> = Vec::<u8>::from([typ, 0, 0, 0, 0, 0, 0, 0].as_ref());
                (RecordType::UnknownType, EasyBuf::from(out))
            },
            _ => {
                let msg = format!("illegal record {:?} from FastCGI server", msg.body);
                panic!(msg);
            }
        };

        let header = FastcgiRecordHeader {
            version: FASTCGI_VERSION,
            record_type: record_type as u8,
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
