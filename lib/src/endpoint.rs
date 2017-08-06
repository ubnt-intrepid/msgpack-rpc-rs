use std::io;
use std::sync::Arc;
use futures::{Future, Stream, Sink};
use futures::future::Then;
use futures::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use tokio_core::reactor::Handle;
use tokio_proto::BindServer;
use tokio_service::Service;
use rmpv::Value;

use super::client::Client;
use super::message::{Request, Response, Notification};
use super::proto::{Proto, Transport};
use super::util::io_error;


/// aaa
pub trait Handler: 'static {
    type RequestFuture: Future<Item = Value, Error = Value>;
    type NotifyFuture: Future<Item = (), Error = ()>;

    ///
    fn handle_request(&self, method: &str, params: Value, client: &Client) -> Self::RequestFuture;

    ///
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn handle_notification(&self, method: &str, params: Value, client: &Client) -> Self::NotifyFuture;
}


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


/// An endpoint of Msgpack-RPC
pub struct Endpoint {
    pub(crate) rx_req: UnboundedReceiver<(u64, Request)>,
    pub(crate) tx_res: UnboundedSender<(u64, Response)>,
    pub(crate) rx_not: UnboundedReceiver<Notification>,
    pub(crate) client: Client,
}


impl Endpoint {
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Spawn tasks to handle services on a event loop of `handle`, with given service handlers.
    pub fn launch<H: Handler>(self, handle: &Handle, handler: H) -> Client {
        let service = Arc::new(HandleService(handler, self.client.clone()));

        let transport = Transport::new(
            self.rx_req.map_err(|()| io_error("rx_req")),
            self.tx_res.sink_map_err(|_| io_error("tx_res")),
        );

        Proto.bind_server(&handle, transport, service.clone());

        handle.spawn(self.rx_not.for_each({
            let client = self.client.clone();
            move |not| {
                service.0.handle_notification(
                    &not.method,
                    not.params,
                    &client,
                )
            }
        }));

        self.client
    }
}
