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
pub use self::client::{Client, ClientFuture};
pub use self::endpoint::{Endpoint, Handler, HandleResult};

use futures::{Stream, Sink};
use futures::sync::mpsc;
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{FramedRead, FramedWrite};

use self::client::NewClient;
use self::distributor::Distributor;
use self::proto::Codec;


/// Create a RPC client and an endpoint, associated with given I/O.
pub fn from_io<T: AsyncRead + AsyncWrite + 'static>(handle: &Handle, io: T) -> Endpoint {
    let (read, write) = io.split();
    from_transport(
        handle,
        FramedRead::new(read, Codec),
        FramedWrite::new(write, Codec),
    )
}

/// Create a RPC client and endpoint, associated with given stream/sink.
pub fn from_transport<T, U>(handle: &Handle, stream: T, sink: U) -> Endpoint
where
    T: Stream<Item = Message> + 'static,
    U: Sink<SinkItem = Message> + 'static,
{
    let (d_tx0, d_rx0) = mpsc::unbounded();
    let (d_tx1, d_rx1) = mpsc::unbounded();
    let (d_tx2, d_rx2) = mpsc::unbounded();
    let (m_tx0, m_rx0) = mpsc::unbounded();
    let (m_tx1, m_rx1) = mpsc::unbounded();
    let (m_tx2, m_rx2) = mpsc::unbounded();

    let client = NewClient {
        tx_req: m_tx0,
        rx_res: d_rx1,
        tx_not: m_tx2,
    };
    let client = client.launch(handle);

    let distributor = Distributor {
        demux: distributor::Demux {
            stream: Some(stream),
            buffer: None,
            tx0: d_tx0,
            tx1: d_tx1,
            tx2: d_tx2,
        },
        mux: distributor::Mux {
            sink,
            buffer: Default::default(),
            rx0: m_rx0,
            rx1: m_rx1,
            rx2: m_rx2,
        },
    };
    distributor.launch(handle);

    Endpoint {
        rx_req: d_rx0,
        tx_res: m_tx1,
        rx_not: d_rx2,
        client,
    }
}
