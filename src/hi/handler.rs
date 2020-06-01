use super::super::*;
use futures::Future;
use std::io;

pub trait FastcgiRequestHandler {
    fn call(&self, request: FastcgiRequest) -> Box<dyn Future<Item=(), Error=io::Error>>;
}
