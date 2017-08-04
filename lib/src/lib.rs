//!
//! An implementation of Msgpack-RPC, based on tokio-proto and rmp.
//!
//! This crate focuses on bi-directional RPC on single I/O.
//!
//! # Example
//!
//! ```ignore
//! // Create a tuple of RPC components from an I/O.
//! let (client, endpoint, distibutor) = msgpack_rpc::from_io(StdioStream::new(4, 4));
//!
//! // You must launch the distributor on certain event loop.
//! // It will executes tasks which process encoding/decoding Msgpack-RPC messages.
//! distributor.launch(&handle);
//!
//! // Launch a client on an event loop.
//! let client = client.launch(&handle);
//!
//! // Call a precedure and receive its response asynchronously.
//! let task = client.request("hello", vec![])
//!     .and_then(|response| {
//!         eprintln!("{:?}", response);
//!         ok(())
//!     });
//!
//! // Start the event loop.
//! core.run(task).unwrap();
//! ```
//!
//! You can serve request/notifications from peer, by using `endpoint`:
//!
//! ```ignore
//! use msgpack_rpc::{Handler, HandleResult};
//!
//! struct RootHandler {
//!     /* ... */
//! }
//!
//! impl Handler for RootHandler {
//!     fn handle_request(&self, method: &str, params: Value) -> HandleResult {
//!         match method {
//!             "func" => ok(Ok(42u64).into()).boxed()
//!             // ...
//!         }
//!     }
//! }
//!
//! // Launch an endpoint service on the event loop of `handle`.
//! // It will spawn a service to handle requests/notifications from a peer.
//! endpoint.launch(&handle, RootHandler { /* ... */ });
//! ```

extern crate bytes;
#[macro_use]
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_proto;
extern crate tokio_service;
extern crate tokio_process;
extern crate rmp;
pub extern crate rmpv;

mod client;
mod distributor;
mod endpoint;
mod message;
mod util;
pub mod io;
pub mod proto;

pub use self::message::Message;
pub use self::client::{Client, NewClient, ClientFuture};
pub use self::distributor::Distributor;
pub use self::endpoint::{Endpoint, Handler, HandleResult};

use futures::{Stream, Sink};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::io::{ReadHalf, WriteHalf};
use tokio_io::codec::{FramedRead, FramedWrite};
use self::proto::Codec;


/// Create a RPC client and an endpoint, associated with given I/O.
pub fn from_io<T>(
    io: T,
) -> (NewClient,
      Endpoint,
      Distributor<FramedRead<ReadHalf<T>, Codec>, FramedWrite<WriteHalf<T>, Codec>>)
where
    T: AsyncRead + AsyncWrite + 'static,
{
    let (read, write) = io.split();
    from_transport(FramedRead::new(read, Codec), FramedWrite::new(write, Codec))
}

/// Create a RPC client and endpoint, associated with given stream/sink.
pub fn from_transport<T, U>(stream: T, sink: U) -> (NewClient, Endpoint, Distributor<T, U>)
where
    T: Stream<Item = Message> + 'static,
    U: Sink<SinkItem = Message> + 'static,
{
    let (distributor, demux_out, mux_in) = distributor::distributor(stream, sink);

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

    (client, endpoint, distributor)
}
