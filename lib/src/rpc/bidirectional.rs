use std::error;
use std::io;
use std::marker::PhantomData;
use futures::{Stream, Sink, Poll, AsyncSink, StartSend};
use futures::sync::mpsc;
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{FramedRead, FramedWrite};

use super::codec::Codec;
use super::message::{Message, Request, Response};


pub struct ClientTransport<T> {
    rx_res: mpsc::Receiver<(u64, Response)>,
    tx_select: mpsc::Sender<Message>,
    _marker: PhantomData<T>,
}

impl<T> Stream for ClientTransport<T> {
    type Item = (u64, Response);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.rx_res.poll().map_err(|_| into_io_error("rx_res"))
    }
}

impl<T> Sink for ClientTransport<T> {
    type SinkItem = (u64, Request);
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        match self.tx_select.start_send(item.into()) {
            Ok(AsyncSink::Ready) => Ok(AsyncSink::Ready),
            Ok(AsyncSink::NotReady(item)) => Ok(AsyncSink::NotReady(item.into())),
            Err(err) => Err(into_io_error(err)),
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.tx_select.poll_complete().map_err(into_io_error)
    }
}



pub struct ServerTransport<T> {
    rx_req: mpsc::Receiver<(u64, Request)>,
    tx_select: mpsc::Sender<Message>,
    _marker: PhantomData<T>,
}

impl<T> Stream for ServerTransport<T> {
    type Item = (u64, Request);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.rx_req.poll().map_err(|()| into_io_error("rx"))
    }
}

impl<T> Sink for ServerTransport<T> {
    type SinkItem = (u64, Response);
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        match self.tx_select.start_send(item.into()) {
            Ok(AsyncSink::Ready) => Ok(AsyncSink::Ready),
            Ok(AsyncSink::NotReady(item)) => Ok(AsyncSink::NotReady(item.into())),
            Err(err) => Err(into_io_error(err)),
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.tx_select.poll_complete().map_err(into_io_error)
    }
}


/// Create a pair of client-side / server-side transports from given asynchronous I/O.
///
/// Note that this function will spawn some background task.
pub fn make_transports<T>(io: T, handle: &Handle) -> (ClientTransport<T>, ServerTransport<T>)
where
    T: AsyncRead + AsyncWrite + 'static,
{
    // TODO: set buffer size
    let (mut tx_req, rx_req) = mpsc::channel(1);
    let (mut tx_res, rx_res) = mpsc::channel(1);
    let (tx_select, rx_select) = mpsc::channel(1);

    let client = ClientTransport {
        rx_res,
        tx_select: tx_select.clone(),
        _marker: PhantomData,
    };

    let server = ServerTransport {
        rx_req,
        tx_select: tx_select.clone(),
        _marker: PhantomData,
    };

    let (read, write) = io.split();
    let stream = FramedRead::new(read, Codec);
    let mut sink = FramedWrite::new(write, Codec);

    // A background task to receive raw messages.
    // It will send received messages to client/server transports.
    // TODO: treat notification message
    handle.spawn(stream.map_err(|_| ()).for_each(
        move |msg| -> Result<(), ()> {
            match msg {
                Message::Request(id, req) => {
                    start_send_until_ready(&mut tx_req, (id, req)).map_err(
                        |_| (),
                    )?;
                }
                Message::Response(id, res) => {
                    start_send_until_ready(&mut tx_res, (id, res)).map_err(
                        |_| (),
                    )?;
                }
                Message::Notification(_not) => (),
            }
            Ok(())
        },
    ));

    // A background task to send messages.
    handle.spawn(rx_select.map_err(|_| ()).for_each(move |msg| {
        fn loop_fn<S: Sink<SinkItem = Message>>(sink: &mut S, msg: Message) -> Result<(), ()> {
            match sink.start_send(msg) {
                Ok(AsyncSink::Ready) => Ok(()),
                Ok(AsyncSink::NotReady(msg)) => loop_fn(sink, msg),
                Err(_) => return Err(()),
            }
        }
        loop_fn(&mut sink, msg)
    }));

    (client, server)
}


fn start_send_until_ready<S: Sink>(
    sink: &mut S,
    mut item: S::SinkItem,
) -> Result<(), S::SinkError> {
    loop {
        item = match sink.start_send(item)? {
            AsyncSink::Ready => break Ok(()),
            AsyncSink::NotReady(item) => item,
        }
    }
}

fn into_io_error<E: Into<Box<error::Error + Send + Sync>>>(err: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}
