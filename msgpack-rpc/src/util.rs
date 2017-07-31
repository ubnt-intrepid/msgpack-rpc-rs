use std::error;
use std::io;
use futures::{Sink, AsyncSink};

pub fn start_send_until_ready<S: Sink>(
    sink: &mut S,
    mut item: S::SinkItem,
) -> Result<(), S::SinkError> {
    eprintln!("[debug] entering in start_send_until_ready");
    loop {
        item = match sink.start_send(item)? {
            AsyncSink::Ready => {
                eprintln!("[debug] success to start_send");
                break Ok(());
            }
            AsyncSink::NotReady(item) => {
                eprintln!("[debug] in loop");
                item
            }
        }
    }
}

pub fn into_io_error<E: Into<Box<error::Error + Send + Sync>>>(err: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}
