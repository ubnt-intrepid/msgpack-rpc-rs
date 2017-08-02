//!
//! An implementation of Msgpack-RPC, based on tokio-proto and rmp.
//!

extern crate bytes;
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_proto;
extern crate tokio_service;
extern crate tokio_process;
extern crate rmp;
extern crate rmpv;

mod client;
mod endpoint;
mod message;
mod multiplexer;
mod transport;
mod util;

pub mod io;

pub use rmpv::Value;
pub use self::client::{Client, NewClient, NotifyClient};
pub use self::message::{Message, Request, Response, Notification};
pub use self::endpoint::{Endpoint, Service, NotifyService};

use futures::{Future, Stream, Sink};
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{FramedRead, FramedWrite};
use self::message::Codec;


/// Create a RPC client and an endpoint, associated with given I/O.
pub fn make_providers<T>(io: T, handle: &Handle) -> (NewClient, Endpoint)
where
    T: AsyncRead + AsyncWrite + 'static,
{
    let (read, write) = io.split();
    make_providers_from_pair(read, write, handle)
}


/// Create a RPC client and service creators, with given I/O pair.
pub fn make_providers_from_pair<R, W>(read: R, write: W, handle: &Handle) -> (NewClient, Endpoint)
where
    R: AsyncRead + 'static,
    W: AsyncWrite + 'static,
{
    let stream = FramedRead::new(read, Codec).map_err(|_| ());
    let sink = FramedWrite::new(write, Codec).sink_map_err(|_| ());

    let (demux_out, task_demux) = multiplexer::demux3(stream);
    let (mux_in, mux_out) = multiplexer::mux3();

    handle.spawn(task_demux);
    handle.spawn(sink.send_all(mux_out).map(|_| ()));

    let client = NewClient {
        tx_req: mux_in.0,
        rx_res: demux_out.1,
        tx_not: mux_in.2,
    };
    let endpoint = Endpoint {
        rx_req: demux_out.0,
        tx_res: mux_in.1,
        rx_not: demux_out.2,
    };

    (client, endpoint)
}
