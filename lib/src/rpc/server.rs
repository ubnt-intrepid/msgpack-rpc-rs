use std::io;
use std::sync::Arc;

use futures::{Future, IntoFuture};
use futures::{Stream, Sink, Poll, Async, AsyncSink, StartSend};
use futures::sync::mpsc;
use tokio_core::reactor::{Core, Handle};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::Framed;
use tokio_proto::BindServer;
use tokio_proto::multiplex::ServerProto;
use tokio_service::NewService;

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

    fn notify(&mut self, mut not: Notification) -> io::Result<()> {
        loop {
            not = match self.tx_notify.start_send(not) {
                Ok(AsyncSink::Ready) => break Ok(()),
                Ok(AsyncSink::NotReady(n)) => n,
                Err(_) => {
                    break Err(io::Error::new(
                        io::ErrorKind::Other,
                        "cannot send notification",
                    ))
                }
            }
        }
    }
}

impl<T: AsyncRead + AsyncWrite + 'static> Stream for ServerTransport<T> {
    type Item = (u64, Request);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            match try_ready!(self.inner.poll()) {
                Some(Message::Request(id, req)) => break Ok(Async::Ready(Some((id, req)))),
                Some(Message::Notification(not)) => {
                    self.notify(not)?;
                    continue;
                }
                Some(_) => continue,
                None => break Ok(Async::Ready(None)),
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

impl Proto {
    pub fn serve<T, S, N, NF, Tsk>(self, stream: T, new_service: S, notify_fn: N, task: Tsk)
    where
        T: AsyncRead + AsyncWrite + 'static,
        S: NewService<Request = Request, Response = Response, Error = io::Error> + 'static,
        N: Fn(Notification) -> NF + 'static,
        NF: IntoFuture<Item = (), Error = ()> + 'static,
        Tsk: Future<Item = (), Error = ()>,
    {
        let new_service = Arc::new(new_service);
        self.with_handle(stream, move |_| new_service.clone(), notify_fn, task)
    }

    pub fn with_handle<T, F, S, N, NF, Tsk>(
        self,
        stream: T,
        new_service: F,
        notify_fn: N,
        task: Tsk,
    ) where
        T: AsyncRead + AsyncWrite + 'static,
        F: Fn(&Handle) -> S + 'static,
        S: NewService<Request = Request, Response = Response, Error = io::Error> + 'static,
        N: Fn(Notification) -> NF + 'static,
        NF: IntoFuture<Item = (), Error = ()> + 'static,
        Tsk: Future<Item = (), Error = ()>,
    {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let (transport, rx_notify) = ServerTransport::new(stream);
        handle.spawn(rx_notify.for_each(notify_fn));

        let new_service = new_service(&handle);
        let service = new_service.new_service().unwrap();
        self.bind_server(&handle, transport, service);

        core.run(task).unwrap();
    }
}
