use std::io;
use std::sync::Arc;
use futures::{Future, Stream, Sink, BoxFuture};
use futures::sync::mpsc::{Sender, Receiver};
use tokio_core::reactor::Handle;
use tokio_proto::BindServer;
use tokio_service::Service;
use rmpv::Value;
use super::message::{Request, Response, Notification};
use super::proto::{Proto, Transport};
use super::util::io_error;


#[allow(unused_variables)]
pub trait Handler: 'static {
    ///
    fn handle_request(&self, method: &str, params: Value) -> HandleResult;

    ///
    fn handle_notification(&self, method: &str, params: Value) -> HandleResult<()> {
        ::futures::future::ok(()).boxed()
    }
}

pub type HandleResult<T = Result<Value, Value>> = BoxFuture<T, ()>;


#[derive(Clone)]
struct HandleService(Arc<Handler>);

impl Service for HandleService {
    type Request = Request;
    type Response = Response;
    type Error = io::Error;
    type Future = BoxFuture<Response, io::Error>;
    fn call(&self, req: Request) -> Self::Future {
        self.0
            .handle_request(&req.method, req.params)
            .map(Response::from)
            .map_err(|_| io_error("HandleService::call"))
            .boxed()
    }
}


/// An endpoint of Msgpack-RPC
pub struct Endpoint {
    pub(crate) rx_req: Receiver<(u64, Request)>,
    pub(crate) tx_res: Sender<(u64, Response)>,
    pub(crate) rx_not: Receiver<Notification>,
}


impl Endpoint {
    /// Spawn tasks to handle services on a event loop of `handle`, with given service handlers.
    ///
    pub fn launch<H: Handler>(self, handle: &Handle, handler: H) {
        let Endpoint {
            rx_req,
            tx_res,
            rx_not,
        } = self;

        let handler = HandleService(Arc::new(handler));

        let transport = Transport::new(
            rx_req.map_err(|()| io_error("rx_req")),
            tx_res.sink_map_err(|_| io_error("tx_res")),
        );
        Proto.bind_server(&handle, transport, handler.clone());

        handle.spawn(rx_not.for_each(move |not| {
            handler.0.handle_notification(&not.method, not.params)
        }));
    }
}
