use std::io::{self, Read, Write};
use std::process::Command;
use futures::{Poll, Async};
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_process::{Child, CommandExt};

/// A non-blocking stream to interact with child process.
pub struct ChildProcessStream {
    child: Child,
}

impl ChildProcessStream {
    pub fn from_builder(command: &mut Command, handle: &Handle) -> io::Result<Self> {
        let child = command.spawn_async(handle)?;
        Ok(ChildProcessStream { child })
    }
}

impl Read for ChildProcessStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let stdout = self.child.stdout().as_mut().unwrap();
        stdout.read(buf)
    }
}

impl Write for ChildProcessStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let stdin = self.child.stdin().as_mut().unwrap();
        stdin.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let stdin = self.child.stdin().as_mut().unwrap();
        stdin.flush()
    }
}

impl AsyncRead for ChildProcessStream {}

impl AsyncWrite for ChildProcessStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        Ok(Async::Ready(()))
    }
}
