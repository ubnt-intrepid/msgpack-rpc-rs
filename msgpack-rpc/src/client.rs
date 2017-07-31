use std::io;
use std::marker::PhantomData;
use futures::{Future, Sink};
use futures::sync::mpsc::{Sender, Receiver};
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_proto::BindClient;
use tokio_proto::multiplex::ClientService;
use tokio_service::Service;

use super::transport::{ClientTransport, BidirectionalProto};
use super::message::{Message, Request, Response, Notification};


pub struct Client<T: AsyncRead + AsyncWrite + 'static> {
    inner: ClientService<ClientTransport, BidirectionalProto>,
    tx_select: Sender<Message>,
    handle: Handle,
    _marker: PhantomData<T>,
}

impl<T: AsyncRead + AsyncWrite + 'static> Client<T> {
    pub(super) fn new(
        handle: &Handle,
        rx_res: Receiver<(u64, Response)>,
        tx_select: Sender<Message>,
    ) -> Self {
        let inner = BidirectionalProto.bind_client(
            handle,
            ClientTransport {
                rx_res,
                tx_select: tx_select.clone(),
            },
        );
        Client {
            inner,
            tx_select,
            handle: handle.clone(),
            _marker: PhantomData,
        }
    }

    /// Send a request message to the server, and return a future of its response.
    pub fn request(&self, req: Request) -> Box<Future<Item = Response, Error = io::Error>> {
        Box::new(self.inner.call(req))
    }

    /// Send a notification message to the server.
    pub fn notify(&mut self, not: Notification) {
        self.handle.spawn(
            self.tx_select
                .clone()
                .send(Message::Notification(not))
                .map(|_| ())
                .map_err(|_| ()),
        );
    }
}
