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
mod transport;
mod util;

pub use tokio_service::Service;
pub use rmpv::Value;
pub use self::client::Client;
pub use self::message::{Message, Request, Response, Notification};

use std::io;
use futures::{Future, Stream, Sink};
use futures::sync::mpsc;
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{FramedRead, FramedWrite};
use tokio_proto::BindServer;
use self::message::Codec;
use self::transport::{ServerTransport, BidirectionalProto};


pub trait NotifyService {
    type Error;
    type Future: Future<Item = (), Error = Self::Error>;
    fn call(&self, not: Notification) -> Self::Future;
}


/// Bind server/notification services and create a RPC client, with given I/O.
pub fn start_services<T, S, N>(io: T, handle: &Handle, service: S, n_service: N) -> Client<T>
where
    T: AsyncRead + AsyncWrite + 'static,
    S: Service<Request = Request, Response = Response, Error = io::Error> + 'static,
    N: NotifyService<Error = io::Error> + 'static,
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
    handle.spawn(rx_select.for_each(
        move |msg| do_send(&mut sink, msg).map_err(|_| ()),
    ));

    // notification services
    handle.spawn(rx_not.for_each(move |not| {
        eprintln!("[debug] receive notification: {:?}", not);
        n_service.call(not).map_err(|_| ())
    }));

    // bind server
    BidirectionalProto.bind_server(
        handle,
        ServerTransport {
            rx_req,
            tx_select: tx_select.clone(),
        },
        service,
    );

    Client::bind(handle, rx_res, tx_select)
}


fn do_send<S: Sink>(sink: &mut Option<S>, item: S::SinkItem) -> Result<(), S::SinkError>
where
    S::SinkItem: ::std::fmt::Debug,
{
    let sink_ = sink.take().expect("sink must not be empty.");
    match sink_.send(item).wait() {
        Ok(s) => {
            *sink = Some(s);
            Ok(())
        }
        Err(e) => Err(e),
    }
}
