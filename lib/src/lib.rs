//!
//! An implementation of Msgpack-RPC, based on tokio-proto and rmp.
//!
//! # Example
//!
//! ## Client
//!
//! ```ignore
//! use msgpack_rpc::from_io;
//! // ...
//!
//! let addr = "127.0.0.1:6666".parse().unwrap();
//! let client = TcpStream::connect(&addr, &handle)
//!     .and_then(|stream| {
//!         let (client, _, distibutor) = from_io(stream);
//!         distributor.launch(&handle);
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
//! core.run(task).unwrap();
//! ```
//!
//! ## Server
//!
//! ```ignore
//! use msgpack_rpc::{Handler, HandleResult, from_io};
//! // ...
//!
//! struct RootService {
//!     /* ... */
//! }
//! impl Handler for RootService {
//!     fn handle_request(&self, method: &str, params: Value) -> HandleResult {
//!         match method {
//!             "func" => ok(Ok(42u64).into()).boxed()
//!             // ...
//!         }
//!     }
//! }
//!
//! let addr = "127.0.0.1:6666".parse().unwrap();
//! let listener = TcpListener::bind(&addr, &handle).unwrap();
//! let server = listener.incoming().for_each(move |(stream, _)| {
//!     let (_, endpoint, distributor) = from_io(stream);
//!     distributor.launch(&handle);
//!
//!     let service = RootService { /* ... */ };
//!     endpoint.serve(&handle, service);
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
use tokio_io::codec::{FramedRead, FramedWrite};
use self::proto::Codec;


/// Create a RPC client and an endpoint, associated with given I/O.
pub fn from_io<T>(io: T) -> (NewClient, Endpoint, Distributor)
where
    T: AsyncRead + AsyncWrite + 'static,
{
    let (read, write) = io.split();
    from_transport(FramedRead::new(read, Codec), FramedWrite::new(write, Codec))
}

/// Create a RPC client and endpoint, associated with given stream/sink.
pub fn from_transport<T, U>(stream: T, sink: U) -> (NewClient, Endpoint, Distributor)
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
