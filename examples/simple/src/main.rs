extern crate neovim;
extern crate msgpack_rpc;
extern crate futures;
extern crate tokio_core;

use neovim::io::StdioStream;
use msgpack_rpc::{Request, Response, Service,  make_providers};
use std::io;
use std::time::Duration;
use std::thread;
use futures::Future;
use futures::future::ok;
use futures::sync::oneshot;
use tokio_core::reactor::Core;

struct Handler;
impl Service for Handler {
    type Request = Request;
    type Response = Response;
    type Error = io::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;
    fn call(&self, req: Request) -> Self::Future {
        eprintln!("[debug] {:?}", req);
        match req.method.as_str() {
            "0:function:the_answer" => Box::new(ok(Response::from_ok(42))),
            "0:function:delay" => {
                if req.params.len() < 2 {
                    return Box::new(ok(Response::from_err("less params")));
                }
                let time = match req.params[0].as_i64() {
                    Some(time) => time,
                    None => {
                        return Box::new(ok(Response::from_err("params[0] should be an integer")))
                    }
                };
                let message = match req.params[1].as_str() {
                    Some(message) => message.to_owned(),
                    None => return Box::new(ok(Response::from_err("params[1] should be a string"))),
                };

                let (tx, rx) = oneshot::channel();
                thread::spawn(move || {
                    thread::sleep(Duration::from_secs(time as u64));
                    tx.send(()).unwrap();
                });
                Box::new(rx.and_then(|_| ok(Response::from_ok(message))).map_err(
                    |e| {
                        io::Error::new(io::ErrorKind::Other, e)
                    },
                ))
            }
            m => Box::new(ok(Response::from_err(
                format!("The method is not found: {:?}", m),
            ))),
        }
    }
}


fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let (stream, rx_stdin) = StdioStream::new(4);
    let (_client, server, _notify) = make_providers(stream, &handle);
    server.serve(&handle, Handler);

    core.run(rx_stdin).unwrap();
}
