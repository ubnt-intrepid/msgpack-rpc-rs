//!
//! An implementation of Msgpack-RPC, based on tokio-proto and rmp.
//!
//! # Example
//!
//! ## Client
//!
//! ```ignore
//! use msgpack_rpc::{Request, make_providers};
//! use futures::Future;
//! use futures::future::ok;
//! use tokio_core::net::TcpStream;
//!
//! let addr = "127.0.0.1:6666".parse().unwrap();
//! let client = TcpStream::connect(&addr, &handle)
//!     .and_then(|stream| {
//!         let (client, _) = make_providers(stream, &handle);
//!         client.launch(&handle)
//!     });
//!
//! let task = client.and_then(|client| {
//!     client.request(Request::new("hello", vec![]))
//!         .and_then(|response| {
//!             println!("{:?}", response);
//!             ok(())
//!         })
//!     });
//!
//! core.run(task).unwrap();
//! ```
//!
//! ## Server
//!
//! ```ignore
//! use msgpack_rpc::{
//!     Request, Response, Notification,
//!     Client, Service, NotifyService,
//!     make_providers
//! };
//! use std::io;
//! use futures::{Future, BoxFuture};
//!
//! struct RootService {
//!     client: Client,
//!     /* ... */
//! }
//! impl Service for RootService {
//!     type Request = Request;
//!     type Response = Response;
//!     type Error = io::Error;
//!     type Future = BoxFuture<Self::Response, Self::Error>;
//!     fn call(&self, req: Self::Request) -> Self::Future {
//!         match req.method.as_str() {
//!             "func" => {
//!                 self.client.request(Request::new("hoge", vec![]))
//!                     .and_then(|response| {
//!                         let message = format!("Received: {:?}", response);
//!                         ok(Response::from_ok(message))
//!                     })
//!             }
//!             // ...
//!         }
//!         /* ... */
//!     }
//! }
//!
//! struct RootNotifyService;
//! impl NotifyService for RootNotifyService {
//!     type Item = Notification;
//!     type Error = io::Error;
//!     type Future = BoxFuture<(), Self::Error>;
//!     fn call(&self, not: Self::Item) -> Self::Future {
//!         /* ... */
//!     }
//! }
//!
//! let listen_addr = "127.0.0.1:6666".parse().unwrap();
//! let listener = TcpListener::bind(&addr, &handle).unwrap();
//!
//! let server = listener.incoming().for_each(move |(stream, _)| {
//!     let (_, endpoint) = make_providers(stream, &handle);
//!
//!     client.launch(&handle)
//!         .and_then(move |client| {
//!             let service = RootService {
//!                 client,
//!                 /* ... */
//!             };
//!             endpoint.serve(&handle, service, RootNotifyService);
//!             ok(())
//!         })
//! });
//! core.run(server);
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
extern crate rmpv;

mod client;
mod endpoint;
mod message;
mod multiplexer;
mod util;

pub mod io;
pub mod proto;

pub use rmpv::Value;
pub use self::client::{Client, NewClient};
pub use self::message::{Message, Request, Response, Notification};
pub use self::endpoint::{Endpoint, Service, NotifyService};

use futures::{Future, Stream, Sink};
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{FramedRead, FramedWrite};
use self::proto::Codec;


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
