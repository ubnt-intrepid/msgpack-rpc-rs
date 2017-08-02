use std::io;
use futures::{Stream, Sink, Poll, StartSend};
use tokio_proto::multiplex::{ClientProto, ServerProto};
use super::message::{Request, Response};


pub struct Tie<TStream: Stream, TSink: Sink>(pub TStream, pub TSink);

impl<TStream: Stream, TSink: Sink> Stream for Tie<TStream, TSink> {
    type Item = TStream::Item;
    type Error = TStream::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.0.poll()
    }
}

impl<TStream: Stream, TSink: Sink> Sink for Tie<TStream, TSink> {
    type SinkItem = TSink::SinkItem;
    type SinkError = TSink::SinkError;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        self.1.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.1.poll_complete()
    }
}


pub struct Proto;

impl<T> ClientProto<T> for Proto
where
    T: 'static
        + Stream<Item = (u64, Response), Error = io::Error>
        + Sink<SinkItem = (u64, Request), SinkError = io::Error>,
{
    type Request = Request;
    type Response = Response;
    type Transport = T;
    type BindTransport = io::Result<Self::Transport>;
    fn bind_transport(&self, transport: T) -> Self::BindTransport {
        Ok(transport)
    }
}

impl<T> ServerProto<T> for Proto
where
    T: 'static
        + Stream<Item = (u64, Request), Error = io::Error>
        + Sink<SinkItem = (u64, Response), SinkError = io::Error>,
{
    type Request = Request;
    type Response = Response;
    type Transport = T;
    type BindTransport = io::Result<Self::Transport>;
    fn bind_transport(&self, transport: T) -> Self::BindTransport {
        Ok(transport)
    }
}
