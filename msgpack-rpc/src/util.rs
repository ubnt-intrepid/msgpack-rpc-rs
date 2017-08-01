use std::error;
use std::io;
use futures::{Future, Sink};


pub fn into_io_error<E: Into<Box<error::Error + Send + Sync>>>(err: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}

pub fn do_send_cloned<S: Sink + Clone + 'static>(
    sink: &S,
    item: S::SinkItem,
) -> Box<Future<Item = (), Error = ()>> {
    Box::new(sink.clone().send(item).then(|res| match res {
        Ok(_) => Ok(()),
        Err(_) => Err(()),
    }))
}
