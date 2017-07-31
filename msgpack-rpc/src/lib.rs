//!
//! An implementation of Msgpack-RPC, based on tokio-proto and rmp.
//!
//! Currently, notification messages are not supported.
//!

extern crate bytes;
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_proto;
extern crate tokio_service;
extern crate rmp;
extern crate rmpv;

mod client;
mod message;
mod notify;
mod transport;
mod server;
mod util;

pub use rmpv::Value;
pub use self::client::Client;
pub use self::message::{Message, Request, Response, Notification};
pub use self::notify::{NotifyServer, NotifyService};
pub use self::server::{Service, Server};

use futures::{Stream, Sink};
use futures::sync::mpsc;
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{FramedRead, FramedWrite};
use self::message::Codec;


/// Create a RPC client and service creators, with given I/O.
pub fn make_providers<T>(io: T, handle: &Handle) -> (Client, Server, NotifyServer)
where
    T: AsyncRead + AsyncWrite + 'static,
{
    let (read, write) = io.split();
    make_providers_from_pair(read, write, handle)
}


/// Create a RPC client and service creators, with given I/O pair.
pub fn make_providers_from_pair<R, W>(
    read: R,
    write: W,
    handle: &Handle,
) -> (Client, Server, NotifyServer)
where
    R: AsyncRead + 'static,
    W: AsyncWrite + 'static,
{
    // create read/write pairs.
    // TODO: set buffer size
    let (tx_req, rx_req) = mpsc::channel(1);
    let (tx_res, rx_res) = mpsc::channel(1);
    let (tx_not, rx_not) = mpsc::channel(1);
    let (tx_select, rx_select) = mpsc::channel(1);

    // A background task to receive raw messages.
    // It will send received messages to client/server transports.
    let stream = FramedRead::new(read, Codec).map_err(|_| ());
    handle.spawn(stream.for_each(move |msg| match msg {
        Message::Request(id, req) => util::do_send_cloned(&tx_req, (id, req)),
        Message::Response(id, res) => util::do_send_cloned(&tx_res, (id, res)),
        Message::Notification(not) => util::do_send_cloned(&tx_not, not),
    }));

    // A background task to send messages.
    let mut sink = Some(FramedWrite::new(write, Codec).sink_map_err(|_| ()));
    handle.spawn(rx_select.for_each(move |msg| util::do_send(&mut sink, msg)));

    let client = Client::new(handle, rx_res, tx_select.clone());
    let notify = NotifyServer::new(rx_not);
    let server = Server::new(rx_req, tx_select);

    (client, server, notify)
}
