use std::marker::PhantomData;
use futures::{Future, Stream};
use futures::sync::mpsc::Receiver;
use tokio_core::reactor::Handle;
use super::message::Notification;


pub trait NotifyService {
    type Error;
    type Future: Future<Item = (), Error = Self::Error>;
    fn call(&self, not: Notification) -> Self::Future;
}


pub struct NotifyServer<T> {
    rx_not: Receiver<Notification>,
    _marker: PhantomData<T>,
}

impl<T> NotifyServer<T> {
    pub(super) fn new(rx_not: Receiver<Notification>) -> Self {
        NotifyServer {
            rx_not,
            _marker: PhantomData,
        }
    }

    pub fn serve<S>(self, handle: &Handle, service: S)
    where
        S: NotifyService + 'static,
    {
        let NotifyServer { rx_not, .. } = self;
        handle.spawn(rx_not.for_each(move |not| {
            eprintln!("[debug] receive notification: {:?}", not);
            service.call(not).map_err(|_| ())
        }));
    }
}
