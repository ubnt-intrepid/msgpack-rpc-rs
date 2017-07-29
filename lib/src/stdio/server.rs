use std::marker::PhantomData;
use std::sync::Arc;
use futures::{Future, Then};
use tokio_core::reactor::{Core, Handle};
use tokio_proto::BindServer;
use tokio_service::{Service, NewService};
use super::StdioStream;


fn change_types<A, B, C, D>(r: Result<A, B>) -> Result<C, D>
where
    A: Into<C>,
    B: Into<D>,
{
    match r {
        Ok(e) => Ok(e.into()),
        Err(e) => Err(e.into()),
    }
}

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
        fn(Result<S::Response,S::Error>) -> Result<Response, Error>
    >;

    fn call(&self, req: Self::Request) -> Self::Future {
       self.inner
            .call(req.into())
            .then(change_types)
    }
}


pub struct StdioServer<Kind, P> {
    _marker: PhantomData<Kind>,
    protocol: P,
    chunk_size: usize,
}

impl<Kind, P> StdioServer<Kind, P>
where
    P: BindServer<Kind, StdioStream> + 'static,
{
    pub fn new(protocol: P, chunk_size: usize) -> Self {
        StdioServer {
            protocol,
            chunk_size,
            _marker: PhantomData,
        }
    }

    pub fn serve<S>(self, new_service: S)
    where
        S: NewService + 'static,
        P::ServiceRequest: 'static,
        P::ServiceResponse: 'static,
        P::ServiceError: 'static,
        S::Request: From<P::ServiceRequest>,
        S::Response: Into<P::ServiceResponse>,
        S::Error: Into<P::ServiceError>,
    {
        let new_service = Arc::new(new_service);
        self.with_handle(move |_| new_service.clone())
    }

    pub fn with_handle<F, S>(self, new_service: F)
    where
        F: Fn(&Handle) -> S + 'static,
        S: NewService + 'static,
        P::ServiceRequest: 'static,
        P::ServiceResponse: 'static,
        P::ServiceError: 'static,
        S::Request: From<P::ServiceRequest>,
        S::Response: Into<P::ServiceResponse>,
        S::Error: Into<P::ServiceError>,
    {
        let StdioServer {
            protocol,
            chunk_size,
            ..
        } = self;

        let (stream, rx_finish) = StdioStream::new(chunk_size);

        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let new_service = new_service(&handle);
        let service = new_service.new_service().unwrap();
        protocol.bind_server(
            &handle,
            stream,
            WrapService {
                inner: service,
                _marker: PhantomData,
            },
        );

        core.run(rx_finish).unwrap();
    }
}
