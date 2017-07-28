use std::io::{self, Read, Write, Stdin, Stdout};
use rpc::{Request, Response};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_service::NewService;
use futures::{Poll, Async};


pub struct StdioStream {
    stdin: Stdin,
    stdout: Stdout,
}

impl StdioStream {
    pub fn new() -> Self {
        StdioStream {
            stdin: io::stdin(),
            stdout: io::stdout(),
        }
    }
}

impl Read for StdioStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stdin.read(buf)
    }
}

impl Write for StdioStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stdout.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}


impl AsyncRead for StdioStream {}

impl AsyncWrite for StdioStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        Ok(Async::Ready(()))
    }
}



pub struct NeoVim {}

impl NeoVim {
    pub fn serve<S>()
    where
        S: NewService<Request = Request, Response = Response, Error = io::Error>,
    {
    }
}
