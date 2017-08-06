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
use super::client::{Client, NewClient};
use super::distributor::{Distributor, Demux, Mux};
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


/// An endpoint of Msgpack-RPC
pub struct Endpoint {
    rx_req: UnboundedReceiver<(u64, Request)>,
    tx_res: UnboundedSender<(u64, Response)>,
    rx_not: UnboundedReceiver<Notification>,
    client: Client,
}


impl Endpoint {
    /// Create a RPC client and an endpoint, associated with given I/O.
    pub fn from_io<T: AsyncRead + AsyncWrite + 'static>(handle: &Handle, io: T) -> Self {
        let (read, write) = io.split();
        Self::from_transport(
            handle,
            FramedRead::new(read, Codec),
            FramedWrite::new(write, Codec),
        )
    }

    /// Create a RPC client and endpoint, associated with given stream/sink.
    pub fn from_transport<T, U>(handle: &Handle, stream: T, sink: U) -> Self
    where
        T: Stream<Item = Message> + 'static,
        U: Sink<SinkItem = Message> + 'static,
    {
        let (d_tx0, d_rx0) = mpsc::unbounded();
        let (d_tx1, d_rx1) = mpsc::unbounded();
        let (d_tx2, d_rx2) = mpsc::unbounded();
        let (m_tx0, m_rx0) = mpsc::unbounded();
        let (m_tx1, m_rx1) = mpsc::unbounded();
        let (m_tx2, m_rx2) = mpsc::unbounded();

        let client = NewClient {
            tx_req: m_tx0,
            rx_res: d_rx1,
            tx_not: m_tx2,
        };
        let client = client.launch(handle);

        let distributor = Distributor {
            demux: Demux {
                stream: Some(stream),
                buffer: None,
                tx0: d_tx0,
                tx1: d_tx1,
                tx2: d_tx2,
            },
            mux: Mux {
                sink,
                buffer: Default::default(),
                rx0: m_rx0,
                rx1: m_rx1,
                rx2: m_rx2,
            },
        };
        distributor.launch(handle);

        Endpoint {
            rx_req: d_rx0,
            tx_res: m_tx1,
            rx_not: d_rx2,
            client,
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn into_client(self) -> Client {
        self.client
    }

    /// Spawn tasks to handle services on a event loop of `handle`, with given service handlers.
    pub fn launch<H: Handler>(self, handle: &Handle, handler: H) -> Client {
        let service = Arc::new(HandleService(handler, self.client.clone()));

        let transport = Transport::new(
            self.rx_req.map_err(|()| io_error("rx_req")),
            self.tx_res.sink_map_err(|_| io_error("tx_res")),
        );

        // Spawn services
        Proto.bind_server(&handle, transport, service.clone());
        handle.spawn(self.rx_not.for_each(move |not| service.call_not(not)));

        self.client
    }
}
