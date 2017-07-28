extern crate neovim;
extern crate futures;
extern crate tokio_core;
extern crate tokio_service;

use neovim::rpc::Request;
use neovim::rpc::client::Client;
use futures::Future;
use tokio_core::reactor::Core;
use tokio_service::Service;

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let task = Client::connect("127.0.0.1:6666", &handle).and_then(|client| {
        client.call(Request::new("nvim_get_api_info")).and_then(
            |response| {
                println!("{:?}", response);
                Ok(())
            },
        )
    });
    core.run(task).unwrap();
}
