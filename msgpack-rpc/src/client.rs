use std::io;
use futures::{Future, Sink};
use futures::sync::mpsc::{Sender, Receiver};
use tokio_core::reactor::Handle;
use tokio_proto::BindClient;
use tokio_proto::multiplex::ClientService;
use tokio_service::Service;

use super::transport::{ClientTransport, BidirectionalProto};
use super::message::{Request, Response, Notification};


#[derive(Clone)]
pub struct Client {
    inner: ClientService<ClientTransport, BidirectionalProto>,
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
        let inner = BidirectionalProto.bind_client(
            handle,
            ClientTransport {
                stream: rx_res,
                sink: tx_req,
            },
        );
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
