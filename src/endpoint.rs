use std::io;
use std::sync::Arc;
use futures::{Future, Stream, Sink};
use futures::future::Then;
use futures::sync::mpsc::{self, UnboundedSender, UnboundedReceiver};
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{FramedRead, FramedWrite};
use tokio_proto::BindServer;
use tokio_service::Service;
use rmpv::Value;

use super::Handler;
use super::client::Client;
use super::distributor::{Demux, Mux};
use super::message::{Message, Request, Response, Notification};
use super::proto::{Codec, Proto, Transport};
use super::util::io_error;


struct HandleService<H: Handler>(H, Client);

impl<H: Handler> Service for HandleService<H> {
    type Request = Request;
    type Response = Response;
    type Error = io::Error;
    type Future = Then<
        H::RequestFuture,
        Result<Response, io::Error>,
        fn(Result<Value, Value>) -> Result<Response, io::Error>,
    >;

    fn call(&self, req: Request) -> Self::Future {
        self.0
            .handle_request(&req.method, req.params, &self.1)
            .then(|res| Ok(Response::from(res)))
    }
}

impl<H: Handler> HandleService<H> {
    fn call_not(&self, not: Notification) -> H::NotifyFuture {
        self.0.handle_notification(&not.method, not.params, &self.1)
    }
}


/// An endpoint represents a peer of MessagePack-RPC.
pub struct Endpoint {
    rx_req: UnboundedReceiver<(u64, Request)>,
    tx_res: UnboundedSender<(u64, Response)>,
    rx_not: UnboundedReceiver<Notification>,
    client: Client,
}


impl Endpoint {
    /// Create a RPC endpoint from asyncrhonous I/O.
    pub fn from_io<T: AsyncRead + AsyncWrite + 'static>(handle: &Handle, io: T) -> Self {
        let (read, write) = io.split();
        Self::from_transport(
            handle,
            FramedRead::new(read, Codec),
            FramedWrite::new(write, Codec),
        )
    }

    /// Create a RPC endpoint from a pair of stream/sink.
    pub fn from_transport<T, U>(handle: &Handle, stream: T, sink: U) -> Self
    where
        T: Stream<Item = Message> + 'static,
        U: Sink<SinkItem = Message> + 'static,
    {
        // create wires.
        let (d_tx0, d_rx0) = mpsc::unbounded();
        let (d_tx1, d_rx1) = mpsc::unbounded();
        let (d_tx2, d_rx2) = mpsc::unbounded();
        let (m_tx0, m_rx0) = mpsc::unbounded();
        let (m_tx1, m_rx1) = mpsc::unbounded();
        let (m_tx2, m_rx2) = mpsc::unbounded();

        // start multiplexer/demultiplexer.
        handle.spawn(Demux::new(stream, d_tx0, d_tx1, d_tx2));
        handle.spawn(Mux::new(sink, m_rx0, m_rx1, m_rx2));

        // start client
        let client = Client::new(handle, m_tx0, d_rx1, m_tx2);

        Endpoint {
            rx_req: d_rx0,
            tx_res: m_tx1,
            rx_not: d_rx2,
            client,
        }
    }

    /// Return the reference of `Client` associated with the endpoint.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Return the instance of `Client` associated with the endpoint.
    ///
    /// This function is useful if the endpoint doesn't handle any incoming requests/notifications.
    /// If the endpoint requires to serve requests/notifications, use `Endpoint::serve()` instead.
    pub fn into_client(self) -> Client {
        self.client
    }

    /// Start to serve incoming requests.
    ///
    /// This function does not block current thread, but returns an instance of `Client` associated
    /// with the I/O.
    /// If you want to run the event loop infinitely, use `futures::future::empty()` as follows:
    ///
    /// ```ignore
    /// let mut core = Core::new().unwrap();
    /// endpoint.serve(&core.handle(), foo);
    /// let _: Result<(), ()> = core.run(empty());
    /// ```
    pub fn serve<H: Handler>(self, handle: &Handle, handler: H) -> Client {
        let service = Arc::new(HandleService(handler, self.client.clone()));

        let transport = Transport::new(
            self.rx_req.map_err(|()| io_error("rx_req")),
            self.tx_res.sink_map_err(|_| io_error("tx_res")),
        );

        // Spawn services
        Proto.bind_server(&handle, transport, service.clone());
        handle.spawn(self.rx_not.for_each(move |not| {
            eprintln!("[debug]");
            service.call_not(not)
        }));

        self.client
    }
}
