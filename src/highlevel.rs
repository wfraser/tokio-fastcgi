use futures::Future;

use std::collections::HashMap;
use std::io;

pub struct FastcgiRequest {
    pub request_id: u16,
    pub params: HashMap<Vec<u8>, Vec<u8>>,
    pub body: Box<Future<Item = Vec<u8>, Error = io::Error>>,
}

pub struct FastcgiResponse {
    pub request_id: u16,
    pub app_status: u32,
    pub protocol_status: u8,
    pub body: Box<Future<Item = Vec<u8>, Error = io::Error>>,
}
