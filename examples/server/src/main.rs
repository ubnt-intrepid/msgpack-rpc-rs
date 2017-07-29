extern crate neovim;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_proto;
extern crate tokio_service;
extern crate futures;
extern crate bytes;

use std::io::{self, BufRead, Write, Stdout};
use std::thread;

use futures::{Future, Stream, Sink, Poll, Async, StartSend, AsyncSink, BoxFuture, IntoFuture};
use futures::sync::mpsc;
use tokio_core::reactor::Core;
use tokio_proto::pipeline::ServerProto;
use tokio_proto::BindServer;
use tokio_service::Service;


struct StdioStream {
    rx_stdin: mpsc::Receiver<io::Result<Vec<u8>>>,
    stdout: Stdout,
}

impl StdioStream {
    fn new() -> Self {
        let (mut tx, rx) = mpsc::channel(0);
        thread::spawn(move || {
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                match tx.send(line.map(|line| line.into_bytes())).wait() {
                    Ok(t) => tx = t,
                    Err(_) => break,
                }
            }
        });
        StdioStream {
            rx_stdin: rx,
            stdout: io::stdout(),
        }
    }
}

impl Stream for StdioStream {
    type Item = Vec<u8>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.rx_stdin.poll() {
            Ok(Async::Ready(Some(res))) => res.map(|line| Async::Ready(Some(line))),
            Ok(Async::Ready(None)) => Ok(Async::Ready(None)),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(_) => Err(io::Error::new(io::ErrorKind::Other, "broken channel")),
        }
    }
}

impl Sink for StdioStream {
    type SinkItem = Vec<u8>;
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        match self.stdout.write(&item) {
            Ok(_n) => Ok(AsyncSink::Ready),
            Err(e) => Err(e),
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        match self.stdout.flush() {
            Ok(()) => Ok(Async::Ready(())),
            Err(e) => Err(e),
        }
    }
}

struct DummyTransport(StdioStream);

impl Stream for DummyTransport {
    type Item = String;
    type Error = io::Error;
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.0.poll().map(|poll| match poll {
            Async::Ready(Some(bytes)) => Async::Ready(
                Some(unsafe { String::from_utf8_unchecked(bytes) }),
            ),
            Async::Ready(None) => Async::Ready(None),
            Async::NotReady => Async::NotReady,
        })
    }
}

impl Sink for DummyTransport {
    type SinkItem = String;
    type SinkError = io::Error;
    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        self.0.start_send(item.into_bytes()).map(|s| match s {
            AsyncSink::Ready => AsyncSink::Ready,
            AsyncSink::NotReady(s) => AsyncSink::NotReady(
                unsafe { String::from_utf8_unchecked(s) },
            ),
        })
    }
    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.0.poll_complete()
    }
}

struct Proto;
impl ServerProto<StdioStream> for Proto {
    type Request = String;
    type Response = String;
    type Transport = DummyTransport;
    type BindTransport = io::Result<Self::Transport>;

    fn bind_transport(&self, io: StdioStream) -> Self::BindTransport {
        Ok(DummyTransport(io))
    }
}

struct Echo;

impl Service for Echo {
    type Request = String;
    type Response = String;
    type Error = io::Error;
    type Future = BoxFuture<Self::Response, Self::Error>;
    fn call(&self, req: Self::Request) -> Self::Future {
        Ok(format!("Received: {:?}\n", req)).into_future().boxed()
    }
}

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let stream = StdioStream::new();
    Proto.bind_server(&handle, stream, Echo);

    let (tx, rx) = futures::sync::oneshot::channel();
    let loop_fn = |_tx: futures::sync::oneshot::Sender<()>| loop {};
    thread::spawn(move || { loop_fn(tx); });

    core.run(rx).unwrap();
}
