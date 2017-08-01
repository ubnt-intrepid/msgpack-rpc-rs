use std::io;
use futures::{Future, Stream};
use futures::sync::mpsc::{Sender, Receiver};
use tokio_core::reactor::Handle;
use tokio_proto::BindServer;
use super::message::{Message, Request, Response, Notification};
use super::transport::BidirectionalProto;
use super::transport::ServerTransport;


/// An asynchronous function which takes an `Item` and no return.
pub trait NotifyService {
    /// Inputs handled by the service.
    type Item;
    /// Errors produces by the service.
    type Error;
    /// The future completion.
    type Future: Future<Item = (), Error = Self::Error>;

    /// Process the input, and return a `()` asynchronously
    fn call(&self, item: Self::Item) -> Self::Future;
}

pub use tokio_service::Service;


/// An endpoint of Msgpack-RPC
pub struct Endpoint {
    rx_req: Receiver<(u64, Request)>,
    tx_select: Sender<Message>,
    rx_not: Receiver<Notification>,
}

impl Endpoint {
    pub(super) fn new(
        rx_req: Receiver<(u64, Request)>,
        tx_select: Sender<Message>,
        rx_not: Receiver<Notification>,
    ) -> Self {
        Endpoint {
            rx_req,
            tx_select,
            rx_not,
        }
    }

    /// Start to serve with given services
    pub fn serve<S, N>(self, handle: &Handle, service: S, n_service: N)
    where
        S: Service<Request = Request, Response = Response, Error = io::Error> + 'static,
        N: NotifyService<Item = Notification, Error = io::Error> + 'static,
    {
        let Endpoint {
            rx_req,
            tx_select,
            rx_not,
        } = self;

        BidirectionalProto.bind_server(&handle, ServerTransport { rx_req, tx_select }, service);

        handle.spawn(rx_not.for_each(
            move |not| n_service.call(not).map_err(|_| ()),
        ));
    }
}
