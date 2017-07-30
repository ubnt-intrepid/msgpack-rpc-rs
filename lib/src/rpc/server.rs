use std::io;
use futures::{Stream, Sink, Poll, Async, AsyncSink, StartSend};
use futures::sync::mpsc;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::Framed;
use tokio_proto::multiplex::ServerProto;

use super::codec::Codec;
use super::message::{Message, Request, Response, Notification};


pub struct ServerTransport<T> {
    inner: Framed<T, Codec>,
    tx_notify: mpsc::Sender<Notification>,
}

impl<T: AsyncRead + AsyncWrite + 'static> ServerTransport<T> {
    pub fn new(io: T) -> (Self, mpsc::Receiver<Notification>) {
        let (tx_notify, rx_notify) = mpsc::channel(1);
        let transport = ServerTransport {
            inner: io.framed(Codec),
            tx_notify,
        };
        (transport, rx_notify)
    }
}

impl<T: AsyncRead + AsyncWrite + 'static> Stream for ServerTransport<T> {
    type Item = (u64, Request);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            match try_ready!(self.inner.poll()) {
                Some(Message::Request(id, req)) => return Ok(Async::Ready(Some((id, req)))),
                Some(Message::Notification(mut not)) => {
                    loop {
                        not = match self.tx_notify.start_send(not) {
                            Ok(AsyncSink::Ready) => break,
                            Ok(AsyncSink::NotReady(n)) => n,
                            Err(_) => {
                                return Err(io::Error::new(
                                    io::ErrorKind::Other,
                                    "cannot send notification",
                                ))
                            }
                        };
                    }
                    continue;
                }
                Some(_) => {
                    continue;
                }
                None => return Ok(Async::Ready(None)),
            }
        }
    }
}

impl<T: AsyncRead + AsyncWrite + 'static> Sink for ServerTransport<T> {
    type SinkItem = (u64, Response);
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        match self.inner.start_send(Message::Response(item.0, item.1)) {
            Ok(AsyncSink::Ready) => Ok(AsyncSink::Ready),
            Ok(AsyncSink::NotReady(Message::Response(id, res))) => Ok(
                AsyncSink::NotReady((id, res)),
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

impl<T: AsyncRead + AsyncWrite + 'static> ServerProto<ServerTransport<T>> for Proto {
    type Request = Request;
    type Response = Response;

    type Transport = ServerTransport<T>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: ServerTransport<T>) -> Self::BindTransport {
        Ok(io)
    }
}
