extern crate neovim;
extern crate futures;
extern crate tokio_core;
extern crate tokio_service;

use neovim::rpc::Request;
use neovim::rpc::client::Client;
use neovim::process::ChildStream;

use std::process::{Command, Stdio};
use futures::Future;
use tokio_core::reactor::Core;
use tokio_service::Service;


fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let stream = ChildStream::from_builder(
        Command::new("nvim")
            .arg("--embed")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()),
        &handle,
    ).unwrap();

    let client = Client::new(stream, &handle);
    let task = client
        .call(Request::new("nvim_get_api_info", vec![]))
        .and_then(|response| {
            println!("{:?}", response);
            Ok(())
        });
    core.run(task).unwrap();
}
