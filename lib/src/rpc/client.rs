use std::io;
use std::net::ToSocketAddrs;
use futures::{Future, Stream, Sink, Poll, Async, AsyncSink, StartSend};
use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::Framed;
use tokio_proto::TcpClient;
use tokio_proto::multiplex::{ClientProto, ClientService};
use tokio_service::Service;

use super::codec::Codec;
use super::message::{Message, Request, Response};



pub struct ClientTransport<T> {
    inner: Framed<T, Codec>,
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

impl<T: AsyncRead + AsyncWrite + 'static> ClientProto<T> for Proto {
    type Request = Request;
    type Response = Response;
    type Transport = ClientTransport<T>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(ClientTransport { inner: io.framed(Codec) })
    }
}



pub struct Client<T: AsyncRead + AsyncWrite + 'static> {
    inner: ClientService<T, Proto>,
}

impl Client<TcpStream> {
    /// Create a new RPC client with given IP address and event handle.
    ///
    /// # Panics
    /// This function will panic if `addr` is not convertible to `SocketAddr`.
    pub fn connect<A: ToSocketAddrs>(
        addr: A,
        handle: &Handle,
    ) -> Box<Future<Item = Self, Error = io::Error>> {
        let addr = addr.to_socket_addrs().unwrap().next().unwrap();
        let client = TcpClient::new(Proto).connect(&addr, handle).map(|inner| {
            Client { inner }
        });
        Box::new(client)
    }
}

impl<T: AsyncRead + AsyncWrite + 'static> Service for Client<T> {
    type Request = Request;
    type Response = Response;
    type Error = io::Error;
    type Future = <ClientService<T, Proto> as Service>::Future;

    fn call(&self, req: Self::Request) -> Self::Future {
        self.inner.call(req)
    }
}
