use std::io::{self, Read, Write, Stdin, Stdout};
use rpc::{Request, Response, server};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_service::Service;
use futures::{Poll, Async};
use tokio_core::reactor::Handle;
use tokio_proto::BindServer;

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
        self.stdin.read(&mut buf[0..32])
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
    pub fn serve<S>(self, handle: &Handle, service: S)
    where
        S: Service<Request = Request, Response = Response, Error = io::Error> + 'static,
    {
        let io = StdioStream::new();
        let protocol = server::Proto;
        protocol.bind_server(handle, io, service)
    }
}
