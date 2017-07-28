extern crate futures;
extern crate tokio_io;
extern crate tokio_core;

use std::io::{self, Read, StdinLock};
use tokio_io::AsyncRead;
use tokio_core::reactor::Core;
use futures::Future;

struct Stdin<'a> {
    inner: StdinLock<'a>,
}

impl<'a> Read for Stdin<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}
impl<'a> AsyncRead for Stdin<'a> {}


fn main() {
    let mut core = Core::new().unwrap();

    let stdin = io::stdin();
    let stdin = Stdin { inner: stdin.lock() };

    let task = tokio_io::io::read_to_end(stdin, Vec::new()).and_then(|(_, buf)| {
        println!("{:?}", buf);
        Ok(())
    });

    core.run(task).unwrap();
}
