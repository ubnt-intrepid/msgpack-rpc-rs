use std::error;
use std::io;
use std::marker::PhantomData;
use futures::{Stream, Sink, Poll, Async, AsyncSink, StartSend};
use futures::sync::mpsc;
use tokio_io::{AsyncRead, AsyncWrite};

use super::codec::Transport;
use super::message::{Message, Request, Response};

pub struct ClientTransport<T: AsyncRead + AsyncWrite + 'static> {
    inner: Transport<T>,
    tx_req: mpsc::Sender<(u64, Request)>,
    _rx_res: mpsc::Receiver<(u64, Response)>,
}

impl<T: AsyncRead + AsyncWrite + 'static> ClientTransport<T> {
    fn send_request(&mut self, mut msg: (u64, Request)) -> io::Result<()> {
        loop {
            msg = match self.tx_req.start_send(msg) {
                Ok(AsyncSink::Ready) => break Ok(()),
                Ok(AsyncSink::NotReady(msg)) => msg,
                Err(err) => break Err(io::Error::new(io::ErrorKind::Other, format!("{:?}", err))),
            }
        }
    }
}

impl<T: AsyncRead + AsyncWrite + 'static> Stream for ClientTransport<T> {
    type Item = (u64, Response);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            match try_ready!(self.inner.poll()) {
                Some(Message::Response(id, res)) => break Ok(Async::Ready(Some((id, res)))),
                Some(Message::Request(id, req)) => {
                    self.send_request((id, req))?;
                    continue;
                }
                Some(_) => unreachable!(),
                None => break Ok(Async::Ready(None)),
            }
        }
    }
}

impl<T: AsyncRead + AsyncWrite + 'static> Sink for ClientTransport<T> {
    type SinkItem = (u64, Request);
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        // TODO: send response if rx is not empty
        match self.inner.start_send(Message::Request(item.0, item.1))? {
            AsyncSink::Ready => Ok(AsyncSink::Ready),
            AsyncSink::NotReady(Message::Request(id, req)) => Ok(AsyncSink::NotReady((id, req))),
            AsyncSink::NotReady(_) => unreachable!(),
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.inner.poll_complete()
    }
}


pub struct ServerTransport<T: AsyncRead + AsyncWrite + 'static> {
    tx_res: mpsc::Sender<(u64, Response)>,
    rx_req: mpsc::Receiver<(u64, Request)>,
    _marker: PhantomData<T>,
}

impl<T: AsyncRead + AsyncWrite + 'static> Stream for ServerTransport<T> {
    type Item = (u64, Request);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.rx_req.poll().map_err(|()| {
            io::Error::new(io::ErrorKind::Other, "rx")
        })
    }
}

impl<T: AsyncRead + AsyncWrite + 'static> Sink for ServerTransport<T> {
    type SinkItem = (u64, Response);
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        self.tx_res.start_send(item).map_err(into_io_error)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.tx_res.poll_complete().map_err(into_io_error)
    }
}


fn into_io_error<E: error::Error>(err: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, format!("{:?}", err))
}


pub fn transports<T: AsyncRead + AsyncWrite + 'static>(
    io: T,
) -> (ClientTransport<T>, ServerTransport<T>) {
    let inner = Transport::from(io);
    // TODO: set buffer size
    let (tx_req, rx_req) = mpsc::channel(1);
    let (tx_res, _rx_res) = mpsc::channel(1);
    let client = ClientTransport {
        inner,
        tx_req,
        _rx_res,
    };
    let server = ServerTransport {
        rx_req,
        tx_res,
        _marker: PhantomData,
    };
    (client, server)
}
