extern crate msgpack_rpc;
extern crate futures;
extern crate tokio_core;
extern crate rmpv;
#[macro_use]
extern crate serde_derive;
extern crate serde;

use msgpack_rpc::from_io;
use msgpack_rpc::io::StdioStream;
use futures::future::empty;
use tokio_core::reactor::Core;

mod handler;
use handler::RootHandler;

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    // create a pair of client/endpoint from an asynchronous I/O.
    let (_client, endpoint) = from_io(&handle, StdioStream::new(4, 4));

    // launch the RPC encpoint with given service handlers.
    endpoint.serve(&handle, RootHandler);

    // start event loop infinitely.
    core.run(empty::<(), ()>()).unwrap();
}
