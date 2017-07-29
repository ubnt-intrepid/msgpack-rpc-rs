extern crate neovim;
extern crate futures;
extern crate tokio_service;

use neovim::stdio::StdioServer;
use neovim::rpc::{Request, Response, Value};
use neovim::rpc::server::Proto as MsgpackRPCProto;

use std::io;
use futures::{Future, BoxFuture, IntoFuture};
use tokio_service::Service;


struct Echo;

impl Service for Echo {
    type Request = Request;
    type Response = Response;
    type Error = io::Error;
    type Future = BoxFuture<Self::Response, Self::Error>;
    fn call(&self, req: Self::Request) -> Self::Future {
        eprintln!("[Received: {:?}]", req);
        Ok(Response(Ok(Value::Nil))).into_future().boxed()
    }
}


fn main() {
    let server = StdioServer::new(MsgpackRPCProto, 1);
    server.serve(|| Ok(Echo));
}
