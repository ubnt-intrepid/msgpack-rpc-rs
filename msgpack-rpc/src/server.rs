use std::io;
use futures::sync::mpsc::{Sender, Receiver};
use tokio_core::reactor::Handle;
use tokio_proto::BindServer;
use super::message::{Message, Request, Response};
use super::transport::BidirectionalProto;
use super::transport::ServerTransport;


pub use tokio_service::Service;


pub struct Server {
    rx_req: Receiver<(u64, Request)>,
    tx_select: Sender<Message>,
}

impl Server {
    pub(super) fn new(rx_req: Receiver<(u64, Request)>, tx_select: Sender<Message>) -> Self {
        Server { rx_req, tx_select }
    }

    pub fn serve<S>(self, handle: &Handle, service: S)
    where
        S: Service<Request = Request, Response = Response, Error = io::Error> + 'static,
    {
        let Server { rx_req, tx_select, .. } = self;
        BidirectionalProto.bind_server(&handle, ServerTransport { rx_req, tx_select }, service);
    }
}
