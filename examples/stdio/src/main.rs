extern crate neovim;
extern crate futures;
extern crate tokio_service;

use neovim::io::StdioStream;
use neovim::rpc::{Request, Response};
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
        Ok(Response::from_ok(true)).into_future().boxed()
    }
}


fn main() {
    let (stream, task) = StdioStream::new(4);
    MsgpackRPCProto.serve(
        stream,
        || Ok(Echo),
        |not| {
            eprintln!("[debug] {:?}", not);
            Ok(())
        },
        task.map_err(|_| ()),
    );
}
