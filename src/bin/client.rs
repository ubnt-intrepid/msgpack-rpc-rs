extern crate msgpack_rpc;
extern crate futures;
extern crate tokio_core;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;

use futures::Future;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use msgpack_rpc::{Endpoint, Value};
use structopt::StructOpt;
use std::io;
use std::net::SocketAddr;

#[derive(StructOpt)]
struct Options {
    #[structopt(name = "ADDR")]
    addr: SocketAddr,

    #[structopt(name = "METHOD")]
    method: String,

    #[structopt(name = "ARGS")]
    args: Vec<String>,

    #[structopt(short = "n", long = "notify")]
    notify: bool,
}

fn main() {
    let opt = Options::from_args();

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let task = TcpStream::connect(&opt.addr, &handle).and_then(
        move |stream| -> Box<Future<Item = (), Error = io::Error>> {
            let method = opt.method.as_str();
            let args = Value::Array(opt.args.into_iter().map(Into::into).collect::<Vec<Value>>());

            let client = Endpoint::from_io(&handle, stream).into_client();
            if opt.notify {
                Box::new(client.notify(method, args))
            } else {
                Box::new(client.request(method, args).and_then(|response| {
                    println!("{:?}", response);
                    Ok(())
                }))
            }
        },
    );

    if let Err(e) = core.run(task) {
        eprintln!("failed with: {}", e);
    }
}
