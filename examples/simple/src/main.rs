extern crate neovim;
extern crate msgpack_rpc;
extern crate futures;
extern crate tokio_core;

use neovim::io::ChildProcessStream;
use msgpack_rpc::{Request, Response, Notification, start_services, Service, NotifyService};
use std::io;
use std::process::{Command, Stdio};
use futures::Future;
use futures::future::{ok, FutureResult};
use tokio_core::reactor::Core;

struct Echo;
impl Service for Echo {
    type Request = Request;
    type Response = Response;
    type Error = io::Error;
    type Future = FutureResult<Self::Response, Self::Error>;
    fn call(&self, _: Request) -> Self::Future {
        eprintln!("[debug] entering in Echo::call()");
        ok(Response::from_ok("Hello"))
    }
}

struct Notify;
impl NotifyService for Notify {
    type Error = io::Error;
    type Future = FutureResult<(), Self::Error>;
    fn call(&self, _: Notification) -> Self::Future {
        eprintln!("[debug] entering in Notify::call()");
        ok(())
    }
}


fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let stream = ChildProcessStream::from_builder(
        Command::new("nvim")
            .arg("--embed")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
        &handle,
    ).unwrap();

    let client = start_services(stream, &handle, Echo, Notify);
    let task = client
        .request(Request::new("vim_get_api_info", vec![]))
        .and_then(|res| {
            println!("{:?}", res);
            ok(())
        });

    core.run(task).unwrap();
}
