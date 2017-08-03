extern crate msgpack_rpc;
extern crate futures;
extern crate tokio_core;
extern crate rmpv;
#[macro_use]
extern crate serde_derive;
extern crate serde;

use msgpack_rpc::make_providers;
use msgpack_rpc::io::StdioStream;
use futures::future::empty;
use tokio_core::reactor::Core;

mod handler;
use handler::RootHandler;

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    // create a pair of client/endpoint from an asynchronous I/O.
    let (_client, endpoint) = make_providers(StdioStream::new(4, 4), &handle);

    // launch the RPC encpoint with given service handlers.
    endpoint.serve(&handle, RootHandler);

    // start event loop infinitely.
    core.run(empty::<(), ()>()).unwrap();
}
