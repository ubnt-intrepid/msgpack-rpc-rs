use std::io;
use futures::{Future, Stream, Sink, Poll, Async, StartSend};
use futures::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use futures::sync::oneshot;
use tokio_core::reactor::{Handle, Remote};
use tokio_proto::BindClient;
use tokio_proto::multiplex::ClientService;
use tokio_service::Service;
use rmpv::Value;

use super::message;
use super::util::io_error;


struct ClientTransport {
    stream: UnboundedReceiver<(u64, message::Response)>,
    sink: UnboundedSender<(u64, message::Request)>,
}

impl Stream for ClientTransport {
    type Item = (u64, message::Response);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.stream.poll().map_err(
            |_| io_error("ClientTransport::poll()"),
        )
    }
}

impl Sink for ClientTransport {
    type SinkItem = (u64, message::Request);
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        self.sink.start_send(item).map_err(|_| {
            io_error("ClientTransport::start_send()")
        })
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.sink.poll_complete().map_err(|_| {
            io_error("ClientTransport::poll_complete()")
        })
    }
}


struct ClientProto;

impl ::tokio_proto::multiplex::ClientProto<ClientTransport> for ClientProto {
    type Request = message::Request;
    type Response = message::Response;
    type Transport = ClientTransport;
    type BindTransport = io::Result<Self::Transport>;
    fn bind_transport(&self, transport: Self::Transport) -> Self::BindTransport {
        Ok(transport)
    }
}


/// The return type of `Client::request()`, represents a future of RPC request.
pub struct Response(<ClientService<ClientTransport, ClientProto> as Service>::Future);

impl Future for Response {
    type Item = Result<Value, Value>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let res = try_ready!(self.0.poll());
        Ok(Async::Ready(res.into_inner()))
    }
}


pub struct Ack(oneshot::Receiver<()>);

impl Future for Ack {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll().map_err(
            |e| io_error(format!("Ack::poll(): {:?}", e)),
        )
    }
}



/// A client of Msgpack-RPC
#[derive(Clone)]
pub struct Client {
    inner: ClientService<ClientTransport, ClientProto>,
    tx_not: UnboundedSender<(message::Notification, oneshot::Sender<()>)>,
    handle: Remote,
}

impl Client {
    /// Create a new `Client` with background task spawned on an event loop of `handle`.
    pub fn new(
        handle: &Handle,
        tx_req: UnboundedSender<(u64, message::Request)>,
        rx_res: UnboundedReceiver<(u64, message::Response)>,
        tx_not: UnboundedSender<(message::Notification, oneshot::Sender<()>)>,
    ) -> Self {
        let transport = ClientTransport {
            stream: rx_res,
            sink: tx_req,
        };

        let inner = ClientProto.bind_client(handle, transport);
        Client {
            inner,
            tx_not,
            handle: handle.remote().clone(),
        }
    }

    /// Send a request message to the server, and return a future of its response.
    pub fn request<S: Into<String>, P: Into<Value>>(&self, method: S, params: P) -> Response {
        Response { 0: self.inner.call(message::Request::new(method, params)) }
    }

    /// Send a notification message to the server.
    pub fn notify<S: Into<String>, P: Into<Value>>(&self, method: S, params: P) -> Ack {
        let not = message::Notification::new(method, params);

        let tx = self.tx_not.clone();
        let (tx_done, rx_done) = oneshot::channel();
        self.handle.spawn(|_| {
            tx.send((not, tx_done)).map(|_| ()).map_err(|_| ())
        });

        Ack { 0: rx_done }
    }
}
