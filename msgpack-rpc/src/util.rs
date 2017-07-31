use std::error;
use std::io;
use futures::{Sink, AsyncSink};

pub fn start_send_until_ready<S: Sink>(
    sink: &mut S,
    mut item: S::SinkItem,
) -> Result<(), S::SinkError> {
    loop {
        item = match sink.start_send(item)? {
            AsyncSink::Ready => break Ok(()),
            AsyncSink::NotReady(item) => item,
        }
    }
}

pub fn into_io_error<E: Into<Box<error::Error + Send + Sync>>>(err: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}
