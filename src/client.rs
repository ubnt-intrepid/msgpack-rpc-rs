use std::io;
use futures::{Future, Stream, Sink, Poll, BoxFuture, StartSend};
use futures::sink::SinkMapErr;
use futures::stream::MapErr;
use futures::sync::mpsc::{UnboundedSender, UnboundedReceiver, SendError};
use tokio_core::reactor::{Handle, Remote};
use tokio_proto::BindClient;
use tokio_proto::multiplex::ClientService;
use tokio_service::Service;
use rmpv::Value;

use super::message::{Request, Response, Notification};
use super::proto::{ Proto};
use super::util::io_error;


struct Transport<T: Stream, U: Sink>(T, U);

impl<T: Stream, U: Sink> Stream for Transport<T, U> {
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.0.poll()
    }
}

impl<T: Stream, U: Sink> Sink for Transport<T, U> {
    type SinkItem = U::SinkItem;
    type SinkError = U::SinkError;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        self.1.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.1.poll_complete()
    }
}

type __ClientTransport = Transport<
    MapErr<UnboundedReceiver<(u64, Response)>, fn(()) -> io::Error>,
    SinkMapErr<
        UnboundedSender<(u64, Request)>,
        fn(SendError<(u64, Request)>) -> io::Error,
    >,
>;

impl ::tokio_proto::multiplex::ClientProto<__ClientTransport> for Proto {
    type Request = Request;
    type Response = Response;
    type Transport = __ClientTransport;
    type BindTransport = io::Result<Self::Transport>;
    fn bind_transport(&self, transport: Self::Transport) -> Self::BindTransport {
        Ok(transport)
    }
}



/// The return type of `Client::request()`, represents a future of RPC request.
pub struct ClientFuture(<ClientService<__ClientTransport, Proto> as Service>::Future);

impl Future for ClientFuture {
    type Item = Response;
    type Error = io::Error;
    #[inline(always)]
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
    }
}


/// A client of Msgpack-RPC
#[derive(Clone)]
pub struct Client {
    inner: ClientService<__ClientTransport, Proto>,
    tx_not: UnboundedSender<Notification>,
    handle: Remote,
}

impl Client {
    /// Create a new `Client` with background task spawned on an event loop of `handle`.
    pub fn new(
        handle: &Handle,
        tx_req: UnboundedSender<(u64, Request)>,
        rx_res: UnboundedReceiver<(u64, Response)>,
        tx_not: UnboundedSender<Notification>,
    ) -> Self {
        let transport = __ClientTransport {
            0:rx_res.map_err((|()| io_error("rx_res")) as fn(()) -> io::Error),
            1:tx_req.sink_map_err((|_| io_error("tx_req")) as fn(SendError<(u64, Request)>) -> io::Error),
        };

        let inner = Proto.bind_client(handle, transport);
        Client {
            inner,
            tx_not,
            handle: handle.remote().clone(),
        }
    }

    /// Send a request message to the server, and return a future of its response.
    pub fn request<S: Into<String>, P: Into<Value>>(&self, method: S, params: P) -> ClientFuture {
        ClientFuture { 0: self.inner.call(Request::new(method, params)) }
    }

    /// Send a notification message to the server.
    pub fn notify<S: Into<String>, P: Into<Value>>(
        &self,
        method: S,
        params: P,
    ) -> BoxFuture<(), ()> {
        let tx = self.tx_not.clone();
        let not = Notification::new(method, params);
        tx.send(not)
            .map(|_| {
                eprintln!("[debug]");
                ()
            })
            .map_err(|_| ())
            .boxed()
    }
}
