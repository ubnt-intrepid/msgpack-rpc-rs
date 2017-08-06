use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::ops::{Deref, DerefMut};
use std::process::{Command, Stdio};
use futures::Poll;
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_process::{Child, ChildStdin, ChildStdout, CommandExt};

/// A non-blocking stream to interact with child process.
pub struct ChildProcessStream {
    child: Child,
}

impl ChildProcessStream {
    pub fn launch<S, I, A>(handle: &Handle, program: S, args: I) -> io::Result<Self>
    where
        S: AsRef<OsStr>,
        I: IntoIterator<Item = A>,
        A: AsRef<OsStr>,
    {
        Self::from_builder(
            handle,
            Command::new(program)
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
        )
    }

    pub fn from_builder(handle: &Handle, command: &mut Command) -> io::Result<Self> {
        let child = command.spawn_async(handle)?;
        Ok(ChildProcessStream { child })
    }

    pub fn into_inner(self) -> Child {
        self.child
    }

    fn child_stdin(&mut self) -> io::Result<&mut ChildStdin> {
        self.child.stdin().as_mut().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "stdin is not open")
        })
    }

    fn child_stdout(&mut self) -> io::Result<&mut ChildStdout> {
        self.child.stdout().as_mut().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "stdout is not open")
        })
    }
}

impl From<Child> for ChildProcessStream {
    fn from(child: Child) -> Self {
        ChildProcessStream { child }
    }
}

impl Deref for ChildProcessStream {
    type Target = Child;
    fn deref(&self) -> &Self::Target {
        &self.child
    }
}

impl DerefMut for ChildProcessStream {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.child
    }
}

impl Read for ChildProcessStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.child_stdout()?.read(buf)
    }
}

impl Write for ChildProcessStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.child_stdin()?.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.child_stdin()?.flush()
    }
}

impl AsyncRead for ChildProcessStream {}

impl AsyncWrite for ChildProcessStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        self.child_stdin()?.shutdown()
    }
}
