use std::io;
use futures::{Stream, Sink, Poll, StartSend};
use futures::sync::mpsc::{Sender, Receiver};
use tokio_proto::multiplex::{ClientProto, ServerProto};

use super::message::{Request, Response};
use super::util;


pub struct ClientTransport {
    pub(super) stream: Receiver<(u64, Response)>,
    pub(super) sink: Sender<(u64, Request)>,
}

impl Stream for ClientTransport {
    type Item = (u64, Response);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.stream.poll().map_err(
            |_| util::into_io_error("rx_res"),
        )
    }
}

impl Sink for ClientTransport {
    type SinkItem = (u64, Request);
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        self.sink.start_send(item).map_err(util::into_io_error)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.sink.poll_complete().map_err(util::into_io_error)
    }
}



pub struct ServerTransport {
    pub(super) stream: Receiver<(u64, Request)>,
    pub(super) sink: Sender<(u64, Response)>,
}

impl Stream for ServerTransport {
    type Item = (u64, Request);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.stream.poll().map_err(|()| util::into_io_error("rx_req"))
    }
}

impl Sink for ServerTransport {
    type SinkItem = (u64, Response);
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        self.sink.start_send(item.into()).map_err(
            util::into_io_error,
        )
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.sink.poll_complete().map_err(util::into_io_error)
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
