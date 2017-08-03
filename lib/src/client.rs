use std::io;
use futures::{Future, Stream, Sink};
use futures::sink::SinkMapErr;
use futures::stream::MapErr;
use futures::sync::mpsc::{Sender, Receiver, SendError};
use tokio_core::reactor::{Handle, Remote};
use tokio_proto::BindClient;
use tokio_proto::multiplex::ClientService;
use tokio_service::Service;

use super::message::{Request, Response, Notification};
use super::proto::{self, Proto};
use super::util::io_error;


type Transport = proto::Transport<
    MapErr<Receiver<(u64, Response)>, fn(()) -> io::Error>,
    SinkMapErr<
        Sender<(u64, Request)>,
        fn(SendError<(u64, Request)>) -> io::Error,
    >,
>;


/// A builder of `Client`, which contains channels to interact with I/O.
pub struct NewClient {
    pub(crate) rx_res: Receiver<(u64, Response)>,
    pub(crate) tx_req: Sender<(u64, Request)>,
    pub(crate) tx_not: Sender<Notification>,
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
        let inner_not = NotifyClient {
            tx_not,
            handle: handle.remote().clone(),
        };
        Client { inner, inner_not }
    }
}


#[derive(Clone)]
struct NotifyClient {
    tx_not: Sender<Notification>,
    handle: Remote,
}

impl NotifyClient {
    fn call(&mut self, not: Notification) {
        let tx_not = self.tx_not.clone();
        self.handle.spawn(|_handle| {
            tx_not.send(not).map(|_| ()).map_err(|_| ())
        });
    }
}


/// The return type of `Client::request()`, represents a future of RPC request.
pub type ClientFuture =
    <ClientService<Transport, Proto> as Service>::Future;


/// A client of Msgpack-RPC
#[derive(Clone)]
pub struct Client {
    inner: ClientService<Transport, Proto>,
    inner_not: NotifyClient,
}

impl Client {
    /// Send a request message to the server, and return a future of its response.
    pub fn request(&self, req: Request) -> ClientFuture {
        self.inner.call(req)
    }

    /// Send a notification message to the server.
    pub fn notify(&mut self, not: Notification) {
        self.inner_not.call(not)
    }
}
