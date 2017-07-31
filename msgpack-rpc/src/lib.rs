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

use futures::{Future, Stream, Sink};
use futures::sync::mpsc;
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{FramedRead, FramedWrite};
use self::message::Codec;


/// Create a RPC client and service creators, with given I/O.
pub fn make_providers<T>(io: T, handle: &Handle) -> (Client<T>, Server<T>, NotifyServer<T>)
where
    T: AsyncRead + AsyncWrite + 'static,
{
    let (read, write) = io.split();

    // create channels.
    // TODO: set buffer size
    let (tx_req, rx_req) = mpsc::channel(1);
    let (tx_res, rx_res) = mpsc::channel(1);
    let (tx_not, rx_not) = mpsc::channel(1);
    let (tx_select, rx_select) = mpsc::channel(1);

    // A background task to receive raw messages.
    // It will send received messages to client/server transports.
    let stream = FramedRead::new(read, Codec).map_err(|_| ()).map(|msg| {
        eprintln!("[debug] read: {:?}", msg);
        msg
    });

    handle.spawn(stream.for_each({
        move |msg| {
            eprintln!("[debug] received: {:?}", msg);
            match msg {
                Message::Request(id, req) => {
                    tx_req
                        .clone()
                        .send((id, req))
                        .map(|_| ())
                        .map_err(|_| ())
                        .boxed()
                }
                Message::Response(id, res) => {
                    tx_res
                        .clone()
                        .send((id, res))
                        .map(|_| ())
                        .map_err(|_| ())
                        .boxed()
                }
                Message::Notification(not) => {
                    tx_not.clone().send(not).map(|_| ()).map_err(|_| ()).boxed()
                }
            }
        }
    }));

    // A background task to send messages.
    let mut sink = Some(FramedWrite::new(write, Codec).sink_map_err(|_| ()));
    handle.spawn(rx_select.for_each(move |msg| {
        util::do_send(&mut sink, msg).map_err(|_| ())
    }));

    let client = Client::new(handle, rx_res, tx_select.clone());
    let notify = NotifyServer::new(rx_not);
    let server = Server::new(rx_req, tx_select);

    (client, server, notify)
}
