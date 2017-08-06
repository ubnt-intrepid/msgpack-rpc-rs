//!
//! An implementation of Msgpack-RPC, based on tokio-proto and rmp.
//!
//! This crate focuses on bi-directional RPC on single I/O.
//!
//! # Example
//!
//! ```ignore
//! // Create a client from an I/O.
//! let endpoint = msgpack_rpc::Endpoint::from_io(&handle, StdioStream::new(4, 4));
//! let client = endpoint.into_client();
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
//! use msgpack_rpc::Handler;
//!
//! struct RootHandler {
//!     /* ... */
//! }
//!
//! impl Handler for RootHandler {
//!     type RequestFuture = BoxFuture<Value, Value>;
//!     type NofityFuture = BoxFuture<(), ()>;
//!
//!     fn handle_request(
//!         &self,
//!         method: &str,
//!         params: Value,
//!         client: &Client,
//!     ) -> Self::RequestFuture {
//!         // ...
//!     }
//!
//!     fn handle_notification(
//!         &self,
//!         method: &str,
//!         params: Value,
//!         client: &Client,
//!     ) -> Self::NotifyFuture {
//!         // ...
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
extern crate rmpv;

mod client;
mod distributor;
mod endpoint;
mod message;
mod util;

pub mod io;
pub mod proto;

pub use rmpv::Value;
pub use self::message::Message;
pub use self::client::{Client, ClientFuture};
pub use self::endpoint::Endpoint;


/// A handler of requests/notifications.
pub trait Handler: 'static {
    /// The future returned from `Self::handle_request()`
    type RequestFuture: ::futures::Future<Item = Value, Error = Value>;

    /// The future returned from `Self::handle_notification()`
    type NotifyFuture: ::futures::Future<Item = (), Error = ()>;

    /// Handler function to handle a request.
    fn handle_request(&self, method: &str, params: Value, client: &Client) -> Self::RequestFuture;

    /// Handler function to handle a notification.
    fn handle_notification(
        &self,
        method: &str,
        params: Value,
        client: &Client,
    ) -> Self::NotifyFuture;
}
