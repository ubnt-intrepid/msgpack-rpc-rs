use std::marker::PhantomData;
use std::sync::Arc;
use futures::{Future, IntoFuture, Stream, Then};
use tokio_core::reactor::{Core, Handle};
use tokio_proto::BindServer;
use tokio_service::{Service, NewService};
use super::StdioStream;

use rpc::Notification;
use rpc::server::ServerTransport;


struct WrapService<S, Request, Response, Error> {
    inner: S,
    _marker: PhantomData<fn() -> (Request, Response, Error)>,
}

impl<S, Request, Response, Error> Service for WrapService<S, Request, Response, Error>
where
    S: Service,
    Request: 'static,
    Response: 'static,
    Error: 'static,
    S::Request: From<Request>,
    S::Response: Into<Response>,
    S::Error: Into<Error>,
    S::Future: 'static
{
    type Request = Request;
    type Response = Response;
    type Error = Error;
    type Future = Then<
        S::Future,
        Result<Response, Error>,
        fn(Result<S::Response, S::Error>) -> Result<Response, Error>
    >;

    fn call(&self, req: Self::Request) -> Self::Future {
       self.inner
           .call(req.into())
           .then(|r| r.map(Into::into).map_err(Into::into))
    }
}


pub struct StdioServer<Kind, P> {
    _marker: PhantomData<Kind>,
    protocol: P,
    chunk_size: usize,
}

impl<Kind, P> StdioServer<Kind, P>
where
    P: BindServer<Kind, ServerTransport<StdioStream>> + 'static,
{
    pub fn new(protocol: P, chunk_size: usize) -> Self {
        StdioServer {
            protocol,
            chunk_size,
            _marker: PhantomData,
        }
    }

    pub fn serve<S, N, NF>(self, new_service: S, notify_fn: N)
    where
        S: NewService + 'static,
        P::ServiceRequest: 'static,
        P::ServiceResponse: 'static,
        P::ServiceError: 'static,
        S::Request: From<P::ServiceRequest>,
        S::Response: Into<P::ServiceResponse>,
        S::Error: Into<P::ServiceError>,
        N: Fn(Notification) -> NF + 'static,
        NF: IntoFuture<Item = (), Error = ()> + 'static,
    {
        let new_service = Arc::new(new_service);
        self.with_handle(move |_| new_service.clone(), notify_fn)
    }

    pub fn with_handle<F, S, N, NF>(self, new_service: F, notify_fn: N)
    where
        F: Fn(&Handle) -> S + 'static,
        S: NewService + 'static,
        P::ServiceRequest: 'static,
        P::ServiceResponse: 'static,
        P::ServiceError: 'static,
        S::Request: From<P::ServiceRequest>,
        S::Response: Into<P::ServiceResponse>,
        S::Error: Into<P::ServiceError>,
        N: Fn(Notification) -> NF + 'static,
        NF: IntoFuture<Item = (), Error = ()> + 'static,
    {
        let StdioServer {
            protocol,
            chunk_size,
            ..
        } = self;

        let (stream, rx_finish) = StdioStream::new(chunk_size);
        let (transport, rx_notify) = ServerTransport::new(stream);

        let mut core = Core::new().unwrap();
        let handle = core.handle();

        handle.spawn(rx_notify.for_each(notify_fn));

        let new_service = new_service(&handle);
        let service = new_service.new_service().unwrap();
        protocol.bind_server(
            &handle,
            transport,
            WrapService {
                inner: service,
                _marker: PhantomData,
            },
        );

        core.run(rx_finish).unwrap();
    }
}
