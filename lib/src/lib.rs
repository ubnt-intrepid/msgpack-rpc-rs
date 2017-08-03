//!
//! An implementation of Msgpack-RPC, based on tokio-proto and rmp.
//!
//! # Example
//!
//! ## Client
//!
//! ```ignore
//! use msgpack_rpc::from_io;
//! use futures::future::{Future, ok};
//! use tokio_core::net::TcpStream;
//!
//! let addr = "127.0.0.1:6666".parse().unwrap();
//! let client = TcpStream::connect(&addr, &handle)
//!     .and_then(|stream| {
//!         let (client, _) = from_io(&handle, stream);
//!         client.launch(&handle)
//!     });
//!
//! let task = client.and_then(|client| {
//!     client.request("hello", vec![])
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
//! use msgpack_rpc::{Client, Handler, HandleResult, from_io};
//! use std::io;
//! use futures::{Future, BoxFuture};
//!
//! struct RootService {
//!     client: Client,
//!     /* ... */
//! }
//! impl Handler for RootService {
//!     fn handle_request(&self, method: &str, params: Value) -> HandleResult {
//!         match method {
//!             "func" => {
//!                 self.client.request("hoge", vec![])
//!                     .and_then(|response| {
//!                         let message = format!("Received: {:?}", response);
//!                         ok(Ok(message).into())
//!                     })
//!             }
//!             // ...
//!         }
//!         // ...
//!     }
//! }
//!
//! let listen_addr = "127.0.0.1:6666".parse().unwrap();
//! let listener = TcpListener::bind(&addr, &handle).unwrap();
//!
//! let server = listener.incoming().for_each(move |(stream, _)| {
//!     let (client, endpoint) = from_io(&handle, stream);
//!     client.launch(&handle)
//!         .and_then(move |client| {
//!             let service = RootService {
//!                 client,
//!                 /* ... */
//!             };
//!             endpoint.serve(&handle, service);
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
pub extern crate rmpv;

mod client;
mod endpoint;
mod message;
mod multiplexer;
mod util;
pub mod io;
pub mod proto;

pub use self::message::Message;
pub use self::client::{Client, NewClient, ClientFuture};
pub use self::endpoint::{NewEndpoint, Handler, HandleResult};

use futures::{Future, Stream, Sink};
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{FramedRead, FramedWrite};
use self::proto::Codec;


/// Create a RPC client and an endpoint, associated with given I/O.
pub fn from_io<T>(handle: &Handle, io: T) -> (NewClient, NewEndpoint)
where
    T: AsyncRead + AsyncWrite + 'static,
{
    let (read, write) = io.split();
    from_transport(
        handle,
        FramedRead::new(read, Codec),
        FramedWrite::new(write, Codec),
    )
}

/// Create a RPC client and endpoint, associated with given stream/sink.
pub fn from_transport<T, U>(handle: &Handle, stream: T, sink: U) -> (NewClient, NewEndpoint)
where
    T: Stream<Item = Message> + 'static,
    U: Sink<SinkItem = Message> + 'static,
{
    let (demux_out, task_demux) = multiplexer::demux3(stream.map_err(|_| ()));
    let (mux_in, mux_out) = multiplexer::mux3();

    handle.spawn(task_demux);
    handle.spawn(sink.sink_map_err(|_| ()).send_all(mux_out).map(|_| ()));

    let client = NewClient {
        tx_req: mux_in.0,
        rx_res: demux_out.1,
        tx_not: mux_in.2,
    };
    let endpoint = NewEndpoint {
        rx_req: demux_out.0,
        tx_res: mux_in.1,
        rx_not: demux_out.2,
    };

    (client, endpoint)
}
