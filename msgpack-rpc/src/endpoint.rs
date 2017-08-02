use std::io;
use futures::{Future, Stream, Sink};
use futures::sync::mpsc::{Sender, Receiver};
use tokio_core::reactor::Handle;
use tokio_proto::BindServer;
use super::message::{Request, Response, Notification};
use super::transport::{Proto, Tie};
use super::util;

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
    tx_res: Sender<(u64, Response)>,
    rx_not: Receiver<Notification>,
}


impl Endpoint {
    pub(super) fn new(
        rx_req: Receiver<(u64, Request)>,
        tx_res: Sender<(u64, Response)>,
        rx_not: Receiver<Notification>,
    ) -> Self {
        Endpoint {
            rx_req,
            tx_res,
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
            tx_res,
            rx_not,
        } = self;

        let transport = Tie(
            rx_req.map_err(|()| util::into_io_error("rx_req")),
            tx_res.sink_map_err(|_| util::into_io_error("tx_res")),
        );
        Proto.bind_server(&handle, transport, service);

        handle.spawn(rx_not.for_each(
            move |not| n_service.call(not).map_err(|_| ()),
        ));
    }
}
