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
    fn new(_chunk_size: usize) -> Self {
        let (tx, rx) = mpsc::channel(0);
        thread::spawn(move || Self::stdin_loop(tx));
        StdioStream {
            rx_stdin: rx,
            stdout: io::stdout(),
        }
    }

    fn stdin_loop(mut tx: mpsc::Sender<io::Result<Vec<u8>>>) {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            match tx.send(line.map(|line| line.into_bytes())).wait() {
                Ok(t) => tx = t,
                Err(_) => break,
            }
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
            Err(()) => Err(io::Error::new(io::ErrorKind::Other, "broken channel")),
        }
    }
}

impl Sink for StdioStream {
    type SinkItem = Vec<u8>;
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        // 標準出力への書き出しにおけるブロッキングはここでは考えない（そこまでのオーバヘッドは生じないはず）
        match self.stdout.write_all(&item) {
            Ok(()) => Ok(AsyncSink::Ready),
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



// StdioStream が小分けにして送信してきた標準入力のバイト列をフレーム単位に分割する層。
// ※ いままで Framed + Codec(Encoder/Decoder) を用いて書いていた処理だが、従来通りに書こうとすると io::Read を実装する必要が生じるため今回は使用しない
struct LineTransport(StdioStream);

impl Stream for LineTransport {
    type Item = String;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.0.poll().map(|poll| match poll {
            Async::Ready(Some(bytes)) => {
                // 今回は読み取ったデータが（改行を含んだ）文字列であることが分かっているため
                // そのまま文字列に変換する。
                // プロトコルによっては受信したデータ長をすべて使いきらないため、その場合は
                // 余剰分のデータをバッファに保存するなどの対処をする必要あり
                let line = unsafe { String::from_utf8_unchecked(bytes) };
                Async::Ready(Some(line))
            }
            Async::Ready(None) => Async::Ready(None),
            Async::NotReady => Async::NotReady,
        })
    }
}

impl Sink for LineTransport {
    type SinkItem = String;
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        // 下層に渡すため一旦バイト列に変換する
        let bytes = item.into_bytes();

        self.0.start_send(bytes).map(|s| match s {
            AsyncSink::Ready => AsyncSink::Ready,
            AsyncSink::NotReady(bytes) => {
                // 変換した文字列をもとに戻す必要がある
                let line = unsafe { String::from_utf8_unchecked(bytes) };
                AsyncSink::NotReady(line)
            }
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
    type Transport = LineTransport;
    type BindTransport = io::Result<Self::Transport>;

    fn bind_transport(&self, io: StdioStream) -> Self::BindTransport {
        Ok(LineTransport(io))
    }
}



struct StdioServer {
    stream: StdioStream,
}

impl StdioServer {
    fn new() -> Self {
        StdioServer { stream: StdioStream::new(0) }
    }

    fn run<S>(self, service: S)
    where
        S: Service<Request = String, Response = String, Error = io::Error> + 'static,
    {
        let StdioServer { stream, .. } = self;

        let mut core = Core::new().unwrap();
        let handle = core.handle();

        //
        Proto.bind_server(&handle, stream, service);

        let (tx, rx) = futures::sync::oneshot::channel();
        let loop_fn = |_tx: futures::sync::oneshot::Sender<()>| loop {};
        thread::spawn(move || { loop_fn(tx); });

        core.run(rx).unwrap();
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
    let server = StdioServer::new();
    server.run(Echo);
}
