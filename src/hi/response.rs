use super::super::*;

use futures::{future, stream, BoxFuture, Future, Sink};
use futures::sync::mpsc;
use tokio_core::io::EasyBuf;
use tokio_proto::streaming::Body;

use std::collections::HashMap;
use std::io;

pub struct FastcgiRequest {
    pub role: Role,
    pub params: HashMap<String, String>,
    pub body: Body<FastcgiRecord, io::Error>, // TODO: instead of giving a stream of body records, give a stream of buffers.
    request_id: u16,
    sender: mpsc::Sender<FastcgiRecord>,
}

impl FastcgiRequest {
    pub fn new(
        role: Role,
        params: HashMap<String, String>,
        body: Body<FastcgiRecord, io::Error>,
        request_id: u16,
        sender: mpsc::Sender<FastcgiRecord>,
        ) -> FastcgiRequest
    {
        FastcgiRequest {
            role: role,
            params: params,
            body: body,
            request_id: request_id,
            sender: sender,
        }
    }

    pub fn response(&self) -> FastcgiHeadersResponse {
        FastcgiHeadersResponse::new(self.request_id, self.sender.clone())
    }
}

pub struct FastcgiHeadersResponse {
    sender: mpsc::Sender<FastcgiRecord>,
    request_id: u16,
    headers: HashMap<String, String>,
}

impl FastcgiHeadersResponse {
    fn new(request_id: u16, sender: mpsc::Sender<FastcgiRecord>) -> FastcgiHeadersResponse {
        let mut headers = HashMap::new();
        headers.insert(
            "X-Powered-By".to_owned(),
            concat!("tokio-fastcgi/", env!("CARGO_PKG_VERSION")).to_owned());
        FastcgiHeadersResponse {
            sender:  sender,
            request_id: request_id,
            headers: headers,
        }
    }

    pub fn set_header<K: Into<String>, V: Into<String>>(&mut self, name: K, value: V) {
        self.headers.insert(name.into(), value.into());
    }

    pub fn clear_header(&mut self, name: &str) {
        self.headers.remove(name);
    }

    pub fn send_headers(self) -> BoxFuture<FastcgiBodyResponse, io::Error> {
        debug!("sending headers");
        let mut out = EasyBuf::new();
        {
            let mut buf_mut = out.get_mut();
            for (ref key, ref value) in self.headers {
                buf_mut.extend_from_slice(key.as_bytes());
                buf_mut.extend_from_slice(b": ");
                buf_mut.extend_from_slice(value.as_bytes());
                buf_mut.extend_from_slice(b"\r\n");
            }
            buf_mut.extend_from_slice(b"\r\n");
        }

        let record = FastcgiRecord {
            request_id: self.request_id,
            body: FastcgiRecordBody::Stdout(out),
        };

        let request_id = self.request_id;

        self.sender
            .send(record)
            .map(move |sender| FastcgiBodyResponse::new(request_id, sender))
            .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))
            .boxed()
    }
}

pub struct FastcgiBodyResponse {
    // this is an `Option` just so we can implement `Drop`.
    sender: Option<mpsc::Sender<FastcgiRecord>>,
    request_id: u16,
    pub buffer: Vec<u8>,
}

impl FastcgiBodyResponse {
    fn new(request_id: u16, sender: mpsc::Sender<FastcgiRecord>) -> FastcgiBodyResponse {
        FastcgiBodyResponse {
            sender: Some(sender),
            request_id: request_id,
            buffer: Vec::new(),
        }
    }

    pub fn flush(mut self) -> BoxFuture<FastcgiBodyResponse, io::Error> {
        debug!("flushing body of {} bytes", self.buffer.len());
        let request_id = self.request_id;

        let buffer = std::mem::replace(&mut self.buffer, vec![]);

        let records = buffer
            .chunks(0xFFFF)
            .map(move |slice| {
                Ok(FastcgiRecord {
                    request_id: request_id,
                    body: FastcgiRecordBody::Stdout(EasyBuf::from(slice.to_vec())),
                })
            })
            .collect::<Vec<_>>();

        self.sender
            .take()
            .unwrap()
            .send_all(stream::iter(records))
            .map(move |(stream, _sink)| {
                FastcgiBodyResponse::new(request_id, stream)
            })
            .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))
            .boxed()
    }

    pub fn finish(self) -> BoxFuture<(), io::Error> {
        debug!("finishing body");
        if self.buffer.is_empty() {
            future::ok(()).boxed()
        } else {
            self.flush()
                .map(|_| ())
                .boxed()
        }
    }
}

impl Drop for FastcgiBodyResponse {
    fn drop(&mut self) {
        if !self.buffer.is_empty() {
            warn!("FastcgiBodyResponse dropped with un-flushed buffer of {} bytes!",
                  self.buffer.len());
        }
    }
}