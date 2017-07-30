use std::io::{self, Read, Write};
use std::process::Command;
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_process::{Child, CommandExt};
use futures::{Poll, Async};

pub struct ChildStream {
    child: Child,
}

impl ChildStream {
    pub fn from_builder(command: &mut Command, handle: &Handle) -> io::Result<Self> {
        let child = command.spawn_async(handle)?;
        Ok(ChildStream { child })
    }
}

impl Read for ChildStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let stdout = self.child.stdout().as_mut().unwrap();
        stdout.read(buf)
    }
}

impl Write for ChildStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let stdin = self.child.stdin().as_mut().unwrap();
        stdin.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let stdin = self.child.stdin().as_mut().unwrap();
        stdin.flush()
    }
}

impl AsyncRead for ChildStream {}

impl AsyncWrite for ChildStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        Ok(Async::Ready(()))
    }
}
