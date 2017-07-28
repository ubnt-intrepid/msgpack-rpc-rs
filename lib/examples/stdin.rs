extern crate neovim;
extern crate futures;
extern crate tokio_io;
extern crate tokio_core;

use neovim::nvim::StdioStream;
use tokio_core::reactor::Core;
use futures::Future;

fn main() {
    let stdio = StdioStream::new();

    let mut core = Core::new().unwrap();
    let task = tokio_io::io::read_to_end(stdio, Vec::new()).and_then(|(_, buf)| {
        println!("{:?}", String::from_utf8(buf));
        Ok(())
    });

    core.run(task).unwrap();
}
