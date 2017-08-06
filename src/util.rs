use std::error;
use std::io;

pub fn io_error<E: Into<Box<error::Error + Send + Sync>>>(err: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}
