use futures::{Future, Stream};
use futures::sync::mpsc::Receiver;
use tokio_core::reactor::Handle;
use super::message::Notification;


pub trait NotifyService {
    type Error;
    type Future: Future<Item = (), Error = Self::Error>;
    fn call(&self, not: Notification) -> Self::Future;
}


pub struct NotifyServer {
    rx_not: Receiver<Notification>,
}

impl NotifyServer {
    pub(super) fn new(rx_not: Receiver<Notification>) -> Self {
        NotifyServer { rx_not }
    }

    pub fn serve<S>(self, handle: &Handle, service: S)
    where
        S: NotifyService + 'static,
    {
        let NotifyServer { rx_not, .. } = self;
        handle.spawn(rx_not.for_each(
            move |not| service.call(not).map_err(|_| ()),
        ));
    }
}
