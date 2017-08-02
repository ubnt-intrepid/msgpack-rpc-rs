use std::io;
use futures::{Future, Stream, Sink};
use futures::sink::SinkMapErr;
use futures::stream::MapErr;
use futures::sync::mpsc::{Sender, Receiver, SendError};
use tokio_core::reactor::Handle;
use tokio_proto::BindClient;
use tokio_proto::multiplex::ClientService;
use tokio_service::Service;

use super::transport::{Proto, Tie};
use super::message::{Request, Response, Notification};
use super::util;


type Transport = Tie<
    MapErr<Receiver<(u64, Response)>, fn(()) -> io::Error>,
    SinkMapErr<
        Sender<(u64, Request)>,
        fn(SendError<(u64, Request)>) -> io::Error,
    >,
>;


#[derive(Clone)]
pub struct Client {
    inner: ClientService<Transport, Proto>,
    tx_not: Sender<Notification>,
    handle: Handle,
}

impl Client {
    pub(super) fn new(
        handle: &Handle,
        rx_res: Receiver<(u64, Response)>,
        tx_req: Sender<(u64, Request)>,
        tx_not: Sender<Notification>,
    ) -> Self {
        let transport = Tie(
            rx_res.map_err((|()| util::into_io_error("rx_res")) as fn(()) -> io::Error),
            tx_req.sink_map_err((|_| util::into_io_error("tx_req")) as fn(SendError<(u64, Request)>) -> io::Error),
        );
        let inner = Proto.bind_client(handle, transport);
        Client {
            inner,
            tx_not,
            handle: handle.clone(),
        }
    }

    /// Send a request message to the server, and return a future of its response.
    pub fn request(&self, req: Request) -> Box<Future<Item = Response, Error = io::Error>> {
        Box::new(self.inner.call(req))
    }

    /// Send a notification message to the server.
    pub fn notify(&mut self, not: Notification) {
        self.handle.spawn(
            self.tx_not
                .clone()
                .send(not)
                .map(|_| ())
                .map_err(|_| ()),
        );
    }
}
