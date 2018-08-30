use super::endian::*;
use super::rawstruct::*;
use super::s11n::*;

use byteorder::{ByteOrder, NetworkEndian};
use bytes::BytesMut;
use enum_primitive::FromPrimitive;
use tokio_io::codec::{Decoder, Encoder};

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
    Params(Vec<(BytesMut, BytesMut)>),
    Stdin(BytesMut),
    Stdout(BytesMut),
    Stderr(BytesMut),
    Data(BytesMut),
    GetValues(Vec<BytesMut>),
    GetValuesResult(Vec<(Vec<u8>, Vec<u8>)>),
    UnknownTypeResponse(u8),
    UnknownType(u8, BytesMut), // this one is the the incoming record
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

#[derive(Debug, Default)]
pub struct FastcgiLowlevelCodec;

fn read_header(buf: &mut BytesMut) -> Option<FastcgiRecordHeader> {
    let header_len = size_of::<FastcgiRecordHeader>();
    if buf.len() < header_len {
        debug!("insufficient buffer for header");
        None
    } else {
        // Only borrow from the buffer until we can check the length.
        let header: FastcgiRecordHeader = unsafe { from_bytes(&buf[0..header_len]) };
        let content_length = header.content_length.get() as usize;
        if buf.len() < content_length {
            debug!("insufficient buffer for message");
            None
        } else {
            // Consume from the buffer now.
            Some(unsafe { from_bytes(&buf.split_to(header_len)) })
        }
    }
}

fn read_len(buf: &mut BytesMut) -> usize {
    let first_byte = buf[0];
    if first_byte < 0x80 {
        buf.split_to(1)[0] as usize
    } else {
        NetworkEndian::read_u32(&buf.split_to(4)) as usize & !0x8000_0000
    }
}

fn write_len(buf: &mut BytesMut, len: usize) {
    if len < 0x80 {
        buf.extend_from_slice(&[len as u8]);
    } else if len < 0x8000_0000 {
        let mut bytes = [0u8; 4];
        NetworkEndian::write_u32(bytes.as_mut(), len as u32 | 0x8000_0000);
        buf.extend_from_slice(bytes.as_ref());
    } else {
        panic!("un-encodable name-value pair length: {:#x}", len);
    }
}

fn read_params(buf: &mut BytesMut) -> Vec<(BytesMut, BytesMut)> {
    let mut params = vec![];
    loop {
        if buf.is_empty() {
            break;
        }
        let name_len = read_len(buf);
        let value_len = read_len(buf);
        let name = buf.split_to(name_len);
        let value = buf.split_to(value_len);
        debug!("param ({}, {})",
               String::from_utf8_lossy(&name),
               String::from_utf8_lossy(&value));
        params.push((name, value));
    }
    params
}

fn write_params(params: Vec<(Vec<u8>, Vec<u8>)>) -> BytesMut {
    let mut out = BytesMut::new();
    for (name, value) in params {
        write_len(&mut out, name.len());
        write_len(&mut out, value.len());
        out.extend_from_slice(name.as_slice());
        out.extend_from_slice(value.as_slice());
    }
    out
}

fn read_begin_request_body(buf: &mut BytesMut) -> io::Result<BeginRequest> {
    let len = size_of::<BeginRequestBody>();
    let raw: BeginRequestBody = unsafe { from_bytes(&buf.split_to(len)) };
    let role = match Role::from_u16(raw.role.get()) {
        Some(role) => role,
        None => {
            let msg = format!("unknown role {}", raw.role.get());
            error!("{}", msg);
            return Err(io::Error::new(io::ErrorKind::InvalidData, msg))
        }
    };
    Ok(BeginRequest {
        role,
        keep_connection: (raw.flags & 1) == 1,
    })
}

impl Decoder for FastcgiLowlevelCodec {
    type Item = FastcgiRecord;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
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
        assert!(buf.len() >= content_len); // should have been checked earlier
        let mut content_buf = buf.split_to(content_len);

        let record_type = RecordType::from_u8(header.record_type).unwrap_or_else(|| {
            warn!("unknwon record type {}", header.record_type);
            RecordType::Mystery
        });
        let request_id = header.request_id.get();

        debug!("request id: {}; record type: {:?}, {} bytes of content",
               request_id, record_type, content_len);

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
        buf.split_to(header.padding_length as usize);

        debug!("buffer now has {} bytes", buf.len());

        let message = FastcgiRecord {
            request_id,
            body,
        };

        Ok(Some(message))
    }
}

impl Encoder for FastcgiLowlevelCodec {
    type Item = FastcgiRecord;
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
        let (record_type, data): (RecordType, BytesMut) = match msg.body {
            FastcgiRecordBody::Stdout(buf) => (RecordType::Stdout, buf),
            FastcgiRecordBody::Stderr(buf) => (RecordType::Stderr, buf),
            FastcgiRecordBody::EndRequest(end_body) => {
                let s11n_body = EndRequestBody {
                    app_status: NetworkU32::new(end_body.app_status),
                    protocol_status: end_body.protocol_status as u8,
                    reserved: [0u8; 3],
                };
                let buf = BytesMut::from(unsafe { as_bytes(&s11n_body) }.to_vec());
                (RecordType::EndRequest, buf)
            },
            FastcgiRecordBody::GetValuesResult(values) => {
                (RecordType::GetValuesResult, write_params(values))
            },
            FastcgiRecordBody::UnknownTypeResponse(typ) => {
                let out: Vec<u8> = Vec::<u8>::from([typ, 0, 0, 0, 0, 0, 0, 0].as_ref());
                (RecordType::UnknownType, BytesMut::from(out))
            },
            _ => {
                let msg = format!("illegal record {:?} from FastCGI server", msg.body);
                panic!(msg);
            }
        };

        if data.len() > 0xFFFF {
            let msg = format!("{:?} record is too long: {}", record_type, data.len());
            error!("{}", msg);
            return Err(io::Error::new(io::ErrorKind::InvalidInput, msg));
        }

        let header = FastcgiRecordHeader {
            version: FASTCGI_VERSION,
            record_type: record_type as u8,
            request_id: NetworkU16::new(msg.request_id),
            content_length: NetworkU16::new(data.len() as u16),
            padding_length: 0,
            reserved: 0,
        };
        buf.extend_from_slice(unsafe { as_bytes(&header) });
        buf.extend_from_slice(&data);

        Ok(())
    }
}
