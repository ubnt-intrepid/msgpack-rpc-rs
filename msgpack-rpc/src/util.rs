use std::error;
use std::io;
use futures::{Future, Sink};


pub fn into_io_error<E: Into<Box<error::Error + Send + Sync>>>(err: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}


pub fn do_send<S: Sink>(sink: &mut Option<S>, item: S::SinkItem) -> Result<(), S::SinkError>
where
    S::SinkItem: ::std::fmt::Debug,
{
    let sink_ = sink.take().expect("sink must not be empty.");
    match sink_.send(item).wait() {
        Ok(s) => {
            *sink = Some(s);
            Ok(())
        }
        Err(e) => Err(e),
    }
}
