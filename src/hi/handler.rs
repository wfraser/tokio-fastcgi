use super::super::*;
use futures::BoxFuture;
use std::io;

pub trait FastcgiRequestHandler {
    fn call(&self, request_future: BoxFuture<FastcgiRequest, io::Error>)
            -> BoxFuture<FastcgiResponse, io::Error>;
}
