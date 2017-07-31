extern crate neovim;
extern crate msgpack_rpc;
extern crate futures;
extern crate tokio_core;

use neovim::io::ChildProcessStream;
use msgpack_rpc::{Request, Response, Notification, Service, Client, NotifyService, make_providers};
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
        ok(Response::from_ok("Hello"))
    }
}

struct Notify(Client);
impl NotifyService for Notify {
    type Error = io::Error;
    type Future = FutureResult<(), Self::Error>;
    fn call(&self, not: Notification) -> Self::Future {
        eprintln!("Receive a notification from client: {:?}", not);
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

    let (client, server, notify) = make_providers(stream, &handle);
    server.serve(&handle, Echo);
    notify.serve(&handle, Notify(client.clone()));

    let task = client
        .request(Request::new("vim_get_api_info", vec![]))
        .and_then(|res| {
            println!("{:?}", res);
            ok(())
        });

    core.run(task).unwrap();
}
