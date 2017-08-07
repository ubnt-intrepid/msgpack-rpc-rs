use std::io;
use futures::{Future, Stream, Sink, Poll, BoxFuture, StartSend};
use futures::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use futures::sync::oneshot;
use tokio_core::reactor::{Handle, Remote};
use tokio_proto::BindClient;
use tokio_proto::multiplex::ClientService;
use tokio_service::Service;
use rmpv::Value;

use super::message::{Request, Response, Notification};
use super::util::io_error;


struct ClientTransport {
    stream: UnboundedReceiver<(u64, Response)>,
    sink: UnboundedSender<(u64, Request)>,
}

impl Stream for ClientTransport {
    type Item = (u64, Response);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.stream.poll().map_err(
            |_| io_error("ClientTransport::poll()"),
        )
    }
}

impl Sink for ClientTransport {
    type SinkItem = (u64, Request);
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
    type Request = Request;
    type Response = Response;
    type Transport = ClientTransport;
    type BindTransport = io::Result<Self::Transport>;
    fn bind_transport(&self, transport: Self::Transport) -> Self::BindTransport {
        Ok(transport)
    }
}


/// The return type of `Client::request()`, represents a future of RPC request.
pub struct ClientFuture(<ClientService<ClientTransport, ClientProto> as Service>::Future);

impl Future for ClientFuture {
    type Item = Response;
    type Error = io::Error;

    #[inline(always)]
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
    }
}


/// A client of Msgpack-RPC
#[derive(Clone)]
pub struct Client {
    inner: ClientService<ClientTransport, ClientProto>,
    tx_not: UnboundedSender<(Notification, oneshot::Sender<()>)>,
    handle: Remote,
}

impl Client {
    /// Create a new `Client` with background task spawned on an event loop of `handle`.
    pub fn new(
        handle: &Handle,
        tx_req: UnboundedSender<(u64, Request)>,
        rx_res: UnboundedReceiver<(u64, Response)>,
        tx_not: UnboundedSender<(Notification, oneshot::Sender<()>)>,
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
    pub fn request<S: Into<String>, P: Into<Value>>(&self, method: S, params: P) -> ClientFuture {
        ClientFuture { 0: self.inner.call(Request::new(method, params)) }
    }

    /// Send a notification message to the server.
    pub fn notify<S: Into<String>, P: Into<Value>>(
        &self,
        method: S,
        params: P,
    ) -> BoxFuture<(), ()> {
        let tx = self.tx_not.clone();
        let not = Notification::new(method, params);
        let (otx, orx) = oneshot::channel();
        self.handle.spawn(|_| {
            tx.send((not, otx)).map(|_| ()).map_err(|_| ())
        });
        orx.map_err(|_| ()).boxed()
    }
}
