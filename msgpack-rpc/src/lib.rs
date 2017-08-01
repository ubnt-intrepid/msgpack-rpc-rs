//!
//! An implementation of Msgpack-RPC, based on tokio-proto and rmp.
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
mod endpoint;
mod message;
mod transport;
mod util;

pub use rmpv::Value;
pub use self::client::Client;
pub use self::message::{Message, Request, Response, Notification};
pub use self::endpoint::{Endpoint, Service, NotifyService};

use futures::{Future, Stream, Sink};
use futures::sync::mpsc::{self, Sender};
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{FramedRead, FramedWrite};
use self::message::Codec;


/// Create a RPC client and an endpoint, associated with given I/O.
pub fn make_providers<T>(io: T, handle: &Handle) -> (Client, Endpoint)
where
    T: AsyncRead + AsyncWrite + 'static,
{
    let (read, write) = io.split();
    make_providers_from_pair(read, write, handle)
}


/// Create a RPC client and service creators, with given I/O pair.
pub fn make_providers_from_pair<R, W>(read: R, write: W, handle: &Handle) -> (Client, Endpoint)
where
    R: AsyncRead + 'static,
    W: AsyncWrite + 'static,
{
    // TODO: set buffer size

    // A background task to receive raw messages.
    // It will send received messages to client/server transports.
    let (tx_req, rx_req) = mpsc::channel(1);
    let (tx_res, rx_res) = mpsc::channel(1);
    let (tx_not, rx_not) = mpsc::channel(1);
    let mux = Multiplexer::new(tx_req, tx_res, tx_not);
    let stream = FramedRead::new(read, Codec).map_err(|_| ());
    handle.spawn(stream.for_each(move |msg| mux.send(msg)));

    // A background task to send messages.
    let (tx_select, rx_select) = mpsc::channel(1);
    let sink = FramedWrite::new(write, Codec).sink_map_err(|_| ());
    handle.spawn(sink.send_all(rx_select).map(|_| ()));

    let client = Client::new(handle, rx_res, tx_select.clone());
    let endpoint = Endpoint::new(rx_req, tx_select, rx_not);

    (client, endpoint)
}


struct Multiplexer {
    req: Sender<(u64, Request)>,
    res: Sender<(u64, Response)>,
    not: Sender<Notification>,
}

impl Multiplexer {
    fn new(
        req: Sender<(u64, Request)>,
        res: Sender<(u64, Response)>,
        not: Sender<Notification>,
    ) -> Self {
        Multiplexer { req, res, not }
    }

    fn send(&self, msg: Message) -> Box<Future<Item = (), Error = ()>> {
        match msg {
            Message::Request(id, req) => util::do_send_cloned(&self.req, (id, req)),
            Message::Response(id, res) => util::do_send_cloned(&self.res, (id, res)),
            Message::Notification(not) => util::do_send_cloned(&self.not, not),
        }
    }
}
