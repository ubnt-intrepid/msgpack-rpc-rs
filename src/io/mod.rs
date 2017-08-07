//!
//! definition of I/O streams and helper functions.
//!

mod stdio;
mod process;

pub use self::stdio::StdioStream;
pub use self::process::ChildProcessStream;

use std::sync::Arc;
use futures::Stream;
use futures::future::empty;
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use super::{Endpoint, Handler};

/// Run the RPC server on standard input/standard output, with given handler.
pub fn run_stdio<H: Handler>(handler: H, chunk_size: usize) {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let io = StdioStream::new(chunk_size);
    let endpoint = Endpoint::from_io(&handle, io);

    endpoint.serve(&handle, handler);
    core.run(empty::<(), ()>()).unwrap();
}

/// Run the RPC server on TCP, with given handler.
pub fn run_tcp<H: Handler>(handler: H, addr: &str) {
    let handler = Arc::new(handler);

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let addr = addr.parse().unwrap();
    let listener = TcpListener::bind(&addr, &handle).unwrap();

    let server = listener.incoming().for_each(move |(sock, _addr)| {
        let endpoint = Endpoint::from_io(&handle, sock);
        endpoint.serve(&handle, handler.clone());
        Ok(())
    });

    core.run(server).unwrap();
}
