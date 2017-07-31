use std::io;
use futures::{Stream, Sink, Poll, AsyncSink, StartSend};
use futures::sync::mpsc::{Sender, Receiver};
use tokio_proto::multiplex::{ClientProto, ServerProto};

use super::message::{Message, Request, Response};
use super::util;


pub struct ClientTransport {
    pub(super) rx_res: Receiver<(u64, Response)>,
    pub(super) tx_select: Sender<Message>,
}

impl Stream for ClientTransport {
    type Item = (u64, Response);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.rx_res.poll().map_err(
            |_| util::into_io_error("rx_res"),
        )
    }
}

impl Sink for ClientTransport {
    type SinkItem = (u64, Request);
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        match self.tx_select.start_send(item.into()) {
            Ok(AsyncSink::Ready) => Ok(AsyncSink::Ready),
            Ok(AsyncSink::NotReady(item)) => Ok(AsyncSink::NotReady(item.into())),
            Err(err) => Err(util::into_io_error(err)),
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.tx_select.poll_complete().map_err(util::into_io_error)
    }
}



pub struct ServerTransport {
    pub(super) rx_req: Receiver<(u64, Request)>,
    pub(super) tx_select: Sender<Message>,
}

impl Stream for ServerTransport {
    type Item = (u64, Request);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.rx_req.poll().map_err(|()| util::into_io_error("rx_req"))
    }
}

impl Sink for ServerTransport {
    type SinkItem = (u64, Response);
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        match self.tx_select.start_send(item.into()) {
            Ok(AsyncSink::Ready) => Ok(AsyncSink::Ready),
            Ok(AsyncSink::NotReady(item)) => Ok(AsyncSink::NotReady(item.into())),
            Err(err) => Err(util::into_io_error(err)),
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.tx_select.poll_complete().map_err(util::into_io_error)
    }
}


pub struct BidirectionalProto;

impl ClientProto<ClientTransport> for BidirectionalProto {
    type Request = Request;
    type Response = Response;
    type Transport = ClientTransport;
    type BindTransport = io::Result<Self::Transport>;
    fn bind_transport(&self, transport: ClientTransport) -> Self::BindTransport {
        Ok(transport)
    }
}

impl ServerProto<ServerTransport> for BidirectionalProto {
    type Request = Request;
    type Response = Response;
    type Transport = ServerTransport;
    type BindTransport = io::Result<Self::Transport>;
    fn bind_transport(&self, transport: ServerTransport) -> Self::BindTransport {
        Ok(transport)
    }
}
