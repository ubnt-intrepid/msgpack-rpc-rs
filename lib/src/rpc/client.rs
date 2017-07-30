use std::io;
use futures::{Stream, Sink, Poll, Async, AsyncSink, StartSend};
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::Framed;
use tokio_proto::multiplex::{ClientProto, ClientService};
use tokio_service::Service;

use super::codec::Codec;
use super::message::{Message, Request, Response};



pub struct ClientTransport<T> {
    inner: Framed<T, Codec>,
}

impl<T: AsyncRead + AsyncWrite + 'static> ClientTransport<T> {
    pub fn new(io: T) -> Self {
        ClientTransport { inner: io.framed(Codec) }
    }
}

impl<T: AsyncRead + AsyncWrite + 'static> Stream for ClientTransport<T> {
    type Item = (u64, Response);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match try_ready!(self.inner.poll()) {
            Some(Message::Response(id, res)) => Ok(Async::Ready(Some((id, res)))),
            Some(_) => Ok(Async::NotReady),
            None => Ok(Async::Ready(None)),
        }
    }
}

impl<T: AsyncRead + AsyncWrite + 'static> Sink for ClientTransport<T> {
    type SinkItem = (u64, Request);
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        match self.inner.start_send(Message::Request(item.0, item.1)) {
            Ok(AsyncSink::Ready) => Ok(AsyncSink::Ready),
            Ok(AsyncSink::NotReady(Message::Request(id, req))) => Ok(
                AsyncSink::NotReady((id, req)),
            ),
            Ok(AsyncSink::NotReady(_)) => unreachable!(),
            Err(err) => Err(err),
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.inner.poll_complete()
    }
}


pub struct Proto;

impl<T: AsyncRead + AsyncWrite + 'static> ClientProto<ClientTransport<T>> for Proto {
    type Request = Request;
    type Response = Response;
    type Transport = ClientTransport<T>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: ClientTransport<T>) -> Self::BindTransport {
        Ok(io)
    }
}

use tokio_proto::BindClient;

pub struct Client<T>
where
    T: AsyncRead + AsyncWrite + 'static,
{
    inner: ClientService<ClientTransport<T>, Proto>,
}

impl<T> Client<T>
where
    T: AsyncRead + AsyncWrite + 'static,
{
    pub fn new_service(stream: T, handle: &Handle) -> Self {
        let transport = ClientTransport::new(stream);
        let inner = Proto.bind_client(handle, transport);
        Client { inner }
    }
}

impl<T> Service for Client<T>
where
    T: AsyncRead + AsyncWrite + 'static,
{
    type Request = Request;
    type Response = Response;
    type Error = io::Error;
    type Future = <ClientService<ClientTransport<T>, Proto> as Service>::Future;

    fn call(&self, req: Self::Request) -> Self::Future {
        self.inner.call(req)
    }
}
