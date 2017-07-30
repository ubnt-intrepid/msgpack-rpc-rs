extern crate neovim;
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_service;
extern crate tokio_process;

use neovim::rpc::Request;
use neovim::rpc::client::Client;
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use futures::{Future, Poll, Async};
use tokio_core::reactor::Core;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_service::Service;
use tokio_process::{CommandExt, Child};

struct ChildStream {
    child: Child,
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


fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let child = Command::new("nvim")
        .arg("--embed")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn_async(&handle)
        .unwrap();
    let stream = ChildStream { child };

    let client = Client::new_service(stream, &handle);
    let task = client
        .call(Request::new("nvim_get_api_info", vec![]))
        .and_then(|response| {
            println!("{:?}", response);
            Ok(())
        });
    core.run(task).unwrap();
}
