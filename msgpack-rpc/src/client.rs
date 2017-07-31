use std::io;
use std::marker::PhantomData;
use futures::Future;
use futures::sync::mpsc::{Sender, Receiver};
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_proto::BindClient;
use tokio_proto::multiplex::ClientService;
use tokio_service::Service;

use super::transport::{ClientTransport, BidirectionalProto};
use super::message::{Message, Request, Response, Notification};
use super::util;


pub struct Client<T: AsyncRead + AsyncWrite + 'static> {
    inner: ClientService<ClientTransport, BidirectionalProto>,
    tx_select: Sender<Message>,
    _marker: PhantomData<T>,
}

impl<T: AsyncRead + AsyncWrite + 'static> Client<T> {
    pub(super) fn bind(
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
            _marker: PhantomData,
        }
    }

    /// Send a request message to the server, and return a future of its response.
    pub fn request(&self, req: Request) -> Box<Future<Item = Response, Error = io::Error>> {
        Box::new(self.inner.call(req))
    }

    /// Send a notification message to the server.
    pub fn notify(&mut self, not: Notification) -> io::Result<()> {
        util::start_send_until_ready(&mut self.tx_select, Message::Notification(not))
            .map_err(util::into_io_error)
    }
}
