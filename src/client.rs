use std::io;
use futures::{Future, Stream, Sink, Poll};
use futures::sink::SinkMapErr;
use futures::stream::MapErr;
use futures::sync::mpsc::{UnboundedSender, UnboundedReceiver, SendError};
use tokio_core::reactor::{Handle, Remote};
use tokio_proto::BindClient;
use tokio_proto::multiplex::ClientService;
use tokio_service::Service;
use rmpv::Value;

use super::message::{Request, Response, Notification};
use super::proto::{self, Proto};
use super::util::io_error;


type Transport = proto::Transport<
    MapErr<UnboundedReceiver<(u64, Response)>, fn(()) -> io::Error>,
    SinkMapErr<
        UnboundedSender<(u64, Request)>,
        fn(SendError<(u64, Request)>) -> io::Error,
    >,
>;


/// A builder of `Client`, which contains channels to interact with I/O.
pub struct NewClient {
    pub(crate) rx_res: UnboundedReceiver<(u64, Response)>,
    pub(crate) tx_req: UnboundedSender<(u64, Request)>,
    pub(crate) tx_not: UnboundedSender<Notification>,
}

impl NewClient {
    /// Create a new `Client` with background task spawned on an event loop of `handle`.
    pub fn launch(self, handle: &Handle) -> Client {
        let NewClient {
            rx_res,
            tx_req,
            tx_not,
        } = self;
        let transport = Transport::new(
            rx_res.map_err((|()| io_error("rx_res")) as fn(()) -> io::Error),
            tx_req.sink_map_err((|_| io_error("tx_req")) as fn(SendError<(u64, Request)>) -> io::Error),
        );

        let inner = Proto.bind_client(handle, transport);
        Client {
            inner,
            tx_not,
            handle: handle.remote().clone(),
        }
    }
}


/// The return type of `Client::request()`, represents a future of RPC request.
pub struct ClientFuture(<ClientService<Transport, Proto> as Service>::Future);

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
    inner: ClientService<Transport, Proto>,
    tx_not: UnboundedSender<Notification>,
    handle: Remote,
}

impl Client {
    /// Send a request message to the server, and return a future of its response.
    pub fn request<S: Into<String>, P: Into<Value>>(&self, method: S, params: P) -> ClientFuture {
        ClientFuture { 0: self.inner.call(Request::new(method, params)) }
    }

    /// Send a notification message to the server.
    pub fn notify<S: Into<String>, P: Into<Value>>(&self, method: S, params: P) {
        let tx = self.tx_not.clone();
        let not = Notification::new(method, params);
        self.handle.spawn(move |_handle| {
            tx.send(not).map(|_| ()).map_err(|_| ())
        });
    }
}
