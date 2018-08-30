use super::super::*;

use bytes::BytesMut;
use futures::{future, Future, Sink};
use futures::stream::{self, Stream};
use futures::sync::mpsc;
use tokio_proto::streaming::Body;

use std::collections::HashMap;
use std::io;

pub struct FastcgiRequest {
    pub role: Role,
    pub params: HashMap<String, String>,
    pub body: Box<Stream<Item=BytesMut, Error=io::Error>>,
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
        // The body stream is expected to consist only of Stdin records. Extract the buffers from
        // these and give the handler a stream of those instead. Anything other than a Stdin record
        // results in an error.
        let buf_stream = body.and_then(|record| {
            match record.body {
                FastcgiRecordBody::Stdin(buf) => Ok(buf),
                _ => {
                    let msg = format!("unexpected request body record {:?}", record.body);
                    error!("{}", msg);
                    Err(io::Error::new(io::ErrorKind::InvalidData, msg))
                }
            }
        }).take_while(|buf| Ok(!buf.is_empty())); // empty Stdin record signals the end.

        FastcgiRequest {
            role,
            params,
            body: Box::new(buf_stream),
            request_id,
            sender,
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
            sender,
            request_id,
            headers,
        }
    }

    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    pub fn headers_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.headers
    }

    pub fn set_header<K: Into<String>, V: Into<String>>(&mut self, name: K, value: V) {
        self.headers.insert(name.into(), value.into());
    }

    pub fn clear_header(&mut self, name: &str) {
        self.headers.remove(name);
    }

    pub fn send_headers(self) -> Box<Future<Item=FastcgiBodyResponse, Error=io::Error> + Send> {
        debug!("sending headers");
        let mut out = BytesMut::new();
        for (ref key, ref value) in self.headers {
            out.extend_from_slice(key.as_bytes());
            out.extend_from_slice(b": ");
            out.extend_from_slice(value.as_bytes());
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"\r\n");

        let record = FastcgiRecord {
            request_id: self.request_id,
            body: FastcgiRecordBody::Stdout(out),
        };

        let request_id = self.request_id;

        Box::new(self.sender
            .send(record)
            .map(move |sender| FastcgiBodyResponse::new(request_id, sender))
            .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e)))
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
            request_id,
            buffer: Vec::new(),
        }
    }

    pub fn flush(mut self) -> Box<Future<Item=FastcgiBodyResponse, Error=io::Error>> {
        debug!("flushing body of {} bytes", self.buffer.len());
        let request_id = self.request_id;

        let buffer = std::mem::replace(&mut self.buffer, vec![]);

        let records = buffer
            .chunks(0xFFFF)
            .map(move |slice| {
                FastcgiRecord {
                    request_id,
                    body: FastcgiRecordBody::Stdout(BytesMut::from(slice.to_vec())),
                }
            })
            .collect::<Vec<_>>();

        Box::new(self.sender
            .take()
            .unwrap()
            .send_all(stream::iter_ok(records))
            .map(move |(stream, _sink)| {
                FastcgiBodyResponse::new(request_id, stream)
            })
            .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e)))
    }

    pub fn finish(self) -> Box<Future<Item=(), Error=io::Error>> {
        debug!("finishing body");
        if self.buffer.is_empty() {
            Box::new(future::ok(()))
        } else {
            Box::new(self.flush()
                .map(|_| ()))
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
