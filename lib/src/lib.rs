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
pub use self::endpoint::Endpoint;

use futures::Future;
use rmpv::Value;


/// aaa
pub trait Handler: 'static {
    type RequestFuture: Future<Item = Value, Error = Value>;
    type NotifyFuture: Future<Item = (), Error = ()>;

    ///
    fn handle_request(&self, method: &str, params: Value, client: &Client) -> Self::RequestFuture;

    ///
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn handle_notification(&self, method: &str, params: Value, client: &Client) -> Self::NotifyFuture;
}
