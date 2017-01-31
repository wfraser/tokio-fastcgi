//! s11n == serialization. Structs that match the bytes that make up FastCGI messages.

use super::endian::*;

pub const FASTCGI_VERSION: u8 = 1;

// Variables for the RecordType::GetValues and GetValuesResult records.
pub const FCGI_MAX_CONNS: &'static str = "FCGI_MAX_CONNS";
pub const FCGI_MAX_REQS: &'static str = "FCGI_MAX_REQS";
pub const FCGI_MPXS_CONNS: &'static str = "FCGI_MPXS_CONNS";

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

enum_from_primitive! {
    #[repr(u8)]
    #[derive(Debug, PartialEq)]
    pub enum Role {
        Responder = 1,
        Authorizer = 2,
        Filter = 3,
    }
}

enum_from_primitive! {
    #[repr(u8)]
    #[derive(Debug, PartialEq)]
    pub enum ProtocolStatus {
        RequestComplete = 0,
        CantMultiplexConnections = 1,
        Overloaded = 2,
        UnknownRole = 3,
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct FastcgiRecordHeader {
    pub version: u8,
    pub record_type: u8,
    pub request_id: NetworkU16,
    pub content_length: NetworkU16,
    pub padding_length: u8,
    pub reserved: u8,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct BeginRequestBody {
    pub role: NetworkU16,
    pub flags: u8,
    pub reserved: [u8; 5],
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct EndRequestBody {
    pub app_status: NetworkU32,
    pub protocol_status: u8,
    pub reserved: [u8; 3],
}
