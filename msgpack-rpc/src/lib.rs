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
use futures::sync::mpsc::{self, Sender, Receiver};
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
    let stream = FramedRead::new(read, Codec).map_err(|_| ());
    let sink = FramedWrite::new(write, Codec).sink_map_err(|_| ());

    let DemuxOutput(rx_req, rx_res, rx_not) = demux(stream, &handle);
    let MuxInput(tx_req, tx_res, tx_not) = mux(sink, &handle);

    let client = Client::new(handle, rx_res, tx_req, tx_not);
    let endpoint = Endpoint::new(rx_req, tx_res, rx_not);

    (client, endpoint)
}


struct DemuxOutput(Receiver<(u64, Request)>, Receiver<(u64, Response)>, Receiver<Notification>);

fn demux<S>(stream: S, handle: &Handle) -> DemuxOutput
where
    S: Stream<Item = Message, Error = ()> + 'static,
{
    let (tx_req, rx_req) = mpsc::channel(1);
    let (tx_res, rx_res) = mpsc::channel(1);
    let (tx_not, rx_not) = mpsc::channel(1);

    handle.spawn(stream.for_each(move |msg| match msg {
        Message::Request(id, req) => util::do_send_cloned(&tx_req, (id, req)),
        Message::Response(id, res) => util::do_send_cloned(&tx_res, (id, res)),
        Message::Notification(not) => util::do_send_cloned(&tx_not, not),
    }));

    DemuxOutput(rx_req, rx_res, rx_not)
}


struct MuxInput(Sender<(u64, Request)>, Sender<(u64, Response)>, Sender<Notification>);

fn mux<S>(sink: S, handle: &Handle) -> MuxInput
where
    S: Sink<SinkItem = Message, SinkError = ()> + 'static,
{
    let (tx_req, rx_req) = mpsc::channel(1);
    let (tx_res, rx_res) = mpsc::channel(1);
    let (tx_not, rx_not) = mpsc::channel(1);
    let (tx_mux, rx_mux) = mpsc::channel(1);

    let rx_req = rx_req.map(|(id, req)| Message::Request(id, req));
    let rx_res = rx_res.map(|(id, res)| Message::Response(id, res));
    let rx_not = rx_not.map(|not| Message::Notification(not));

    handle.spawn(
        tx_mux
            .sink_map_err(|_| ())
            .send_all(rx_req.select(rx_res).select(rx_not))
            .map(|_| ()),
    );

    handle.spawn(sink.send_all(rx_mux).map(|_| ()));

    MuxInput(tx_req, tx_res, tx_not)
}
