use super::super::*;
use futures::BoxFuture;
use std::io;

pub trait FastcgiRequestHandler {
    fn call(&self, request: FastcgiRequest) -> BoxFuture<(), io::Error>;
}
