use std::error;
use std::io;
use std::marker::PhantomData;

use futures::{Future, Stream, Sink, Poll, AsyncSink, StartSend};
use futures::sync::mpsc;
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{FramedRead, FramedWrite};
use tokio_proto::{BindClient, BindServer};
use tokio_proto::multiplex::{ClientProto, ClientService, ServerProto};
use tokio_service::Service;

use super::codec::Codec;
use super::message::{Message, Request, Response, Notification};


pub struct ClientTransport {
    rx_res: mpsc::Receiver<(u64, Response)>,
    tx_select: mpsc::Sender<Message>,
}

impl Stream for ClientTransport {
    type Item = (u64, Response);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.rx_res.poll().map_err(|_| into_io_error("rx_res"))
    }
}

impl Sink for ClientTransport {
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



pub struct ServerTransport {
    rx_req: mpsc::Receiver<(u64, Request)>,
    tx_select: mpsc::Sender<Message>,
}

impl Stream for ServerTransport {
    type Item = (u64, Request);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.rx_req.poll().map_err(|()| into_io_error("rx"))
    }
}

impl Sink for ServerTransport {
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


pub struct Client<T: AsyncRead + AsyncWrite + 'static> {
    inner: ClientService<ClientTransport, BidirectionalProto>,
    tx_select: mpsc::Sender<Message>,
    _marker: PhantomData<T>,
}

impl<T: AsyncRead + AsyncWrite + 'static> Client<T> {
    /// Send a request message to the server, and return a future of its response.
    pub fn request(&self, req: Request) -> Box<Future<Item = Response, Error = io::Error>> {
        Box::new(self.inner.call(req))
    }

    /// Send a notification message to the server.
    pub fn notify(&mut self, not: Notification) -> io::Result<()> {
        start_send_until_ready(&mut self.tx_select, Message::Notification(not))
            .map_err(into_io_error)
    }
}


pub trait NotifyService {
    type Error;
    type Future: Future<Item = (), Error = Self::Error>;
    fn call(&self, not: Notification) -> Self::Future;
}


/// Create a pair of client-side / server-side transports from given asynchronous I/O.
///
/// Note that this function will spawn some background task.
pub fn start_services<T, S, N>(io: T, handle: &Handle, service: S, n_service: N) -> Client<T>
where
    T: AsyncRead + AsyncWrite + 'static,
    S: Service<Request = Request, Response = Response, Error = io::Error> + 'static,
    N: NotifyService<Error = io::Error> + 'static,
{
    let (read, write) = io.split();

    // create channels.
    // TODO: set buffer size
    let (tx_req, rx_req) = mpsc::channel(1);
    let (tx_res, rx_res) = mpsc::channel(1);
    let (tx_not, rx_not) = mpsc::channel(1);
    let (tx_select, rx_select) = mpsc::channel(1);

    // A background task to receive raw messages.
    // It will send received messages to client/server transports.
    let stream = FramedRead::new(read, Codec).map_err(|_| ());
    let mut tx_req = tx_req.sink_map_err(|_| ());
    let mut tx_res = tx_res.sink_map_err(|_| ());
    let mut tx_not = tx_not.sink_map_err(|_| ());
    handle.spawn(stream.for_each(move |msg| match msg {
        Message::Request(id, req) => start_send_until_ready(&mut tx_req, (id, req)),
        Message::Response(id, res) => start_send_until_ready(&mut tx_res, (id, res)),
        Message::Notification(not) => start_send_until_ready(&mut tx_not, not),
    }));

    // A background task to send messages.
    let mut sink = FramedWrite::new(write, Codec).sink_map_err(|_| ());
    handle.spawn(rx_select.for_each(
        move |msg| start_send_until_ready(&mut sink, msg),
    ));

    // notification services
    handle.spawn(rx_not.for_each(
        move |not| n_service.call(not).map_err(|_| ()),
    ));

    // bind server
    BidirectionalProto.bind_server(
        handle,
        ServerTransport {
            rx_req,
            tx_select: tx_select.clone(),
        },
        service,
    );

    // create an instance of ClientService
    let inner = BidirectionalProto.bind_client(
        handle,
        ClientTransport {
            rx_res,
            tx_select: tx_select.clone(),
        },
    );
    Client {
        inner,
        tx_select,
        _marker: PhantomData,
    }
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
