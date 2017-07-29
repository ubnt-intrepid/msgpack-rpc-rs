extern crate neovim;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_proto;
extern crate tokio_service;
extern crate futures;
extern crate bytes;

use std::io::{self, Read, Write, Stdout};
use std::marker::PhantomData;
use std::thread;
use std::sync::Arc;

use bytes::BytesMut;
use futures::{Future, Stream, Sink, Poll, Async, StartSend, AsyncSink, BoxFuture, IntoFuture, Then};
use futures::sync::{mpsc, oneshot};
use tokio_core::reactor::{Core, Handle};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_proto::pipeline::ServerProto;
use tokio_proto::BindServer;
use tokio_service::{Service, NewService};


struct StdioStream {
    rx_stdin: mpsc::Receiver<io::Result<Vec<u8>>>,
    stdout: Stdout,
}

impl StdioStream {
    fn new(chunk_size: usize) -> (Self, oneshot::Receiver<()>) {
        assert!(chunk_size > 0);

        let (tx_stdin, rx_stdin) = mpsc::channel(0);
        let (tx_finish, rx_finish) = oneshot::channel::<()>();
        thread::spawn(move || Self::stdin_loop(tx_stdin, tx_finish, chunk_size));

        let stream = StdioStream {
            rx_stdin,
            stdout: io::stdout(),
        };

        (stream, rx_finish)
    }

    fn stdin_loop(
        mut tx_stdin: mpsc::Sender<io::Result<Vec<u8>>>,
        tx_finish: oneshot::Sender<()>,
        chunk_size: usize,
    ) {
        let stdin = io::stdin();
        let mut locked_stdin = stdin.lock();
        loop {
            let mut bytes = vec![0u8; chunk_size];
            match locked_stdin.read_exact(&mut bytes) {
                Ok(()) => {
                    match tx_stdin.send(Ok(bytes)).wait() {
                        Ok(t) => tx_stdin = t,
                        Err(_) => break,
                    }
                }
                Err(err) => {
                    let _ = tx_stdin.send(Err(err)).wait();
                    break;
                }
            }
        }
        let _ = tx_finish.send(());
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

struct Stdio {
    inner: StdioStream,
    buffer: BytesMut,
    is_finished: bool,
}

impl Stdio {
    fn new() -> (Stdio, oneshot::Receiver<()>) {
        let (inner, rx) = StdioStream::new(0);
        let buffer = BytesMut::new();
        let stdio = Stdio {
            inner,
            buffer,
            is_finished: false,
        };
        (stdio, rx)
    }
}

impl Read for Stdio {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.is_finished {
            return Ok(0);
        }

        if self.buffer.len() > 0 {
            // read from buffer.
            let len = std::cmp::min(self.buffer.len(), buf.len());
            buf[0..len].copy_from_slice(&self.buffer[0..len]);
            self.buffer.split_to(len);
            return Ok(len);
        }

        debug_assert_eq!(self.buffer.len(), 0);

        match self.inner.poll() {
            Ok(Async::Ready(Some(bytes))) => {
                let len = std::cmp::min(bytes.len(), buf.len());
                buf[0..len].copy_from_slice(&bytes[0..len]);
                if len < bytes.len() {
                    self.buffer.extend_from_slice(&bytes[len..]);
                }
                Ok(len)
            }
            Ok(Async::Ready(None)) => {
                self.is_finished = true;
                Ok(0)
            }
            Ok(Async::NotReady) => Err(io::Error::new(io::ErrorKind::WouldBlock, "Not ready")),
            Err(err) => Err(err),
        }
    }
}

impl Write for Stdio {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.stdout.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.stdout.flush()
    }
}

impl AsyncRead for Stdio {}

impl AsyncWrite for Stdio {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        Ok(Async::Ready(()))
    }
}



// StdioStream が小分けにして送信してきた標準入力のバイト列をフレーム単位に分割する層。
// ※ いままで Framed + Codec(Encoder/Decoder) を用いて書いていた処理だが、
//    従来通りに書こうとすると io::Read を実装する必要が生じるため今回は使用しない
struct LineTransport<T>(T);

impl<T> Stream for LineTransport<T>
where
    T: 'static
        + Stream<Item = Vec<u8>, Error = io::Error>
        + Sink<SinkItem = Vec<u8>, SinkError = io::Error>,
{
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

impl<T> Sink for LineTransport<T>
where
    T: 'static
        + Stream<Item = Vec<u8>, Error = io::Error>
        + Sink<SinkItem = Vec<u8>, SinkError = io::Error>,
{
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



struct LineProto;

impl<T> ServerProto<T> for LineProto
where
    T: 'static
        + Stream<Item = Vec<u8>, Error = io::Error>
        + Sink<SinkItem = Vec<u8>, SinkError = io::Error>,
{
    type Request = String;
    type Response = String;
    type Transport = LineTransport<T>;
    type BindTransport = io::Result<Self::Transport>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(LineTransport(io))
    }
}



struct StdioServer<Kind, P> {
    _marker: PhantomData<Kind>,
    protocol: P,
}

impl<Kind, P> StdioServer<Kind, P>
where
    P: BindServer<Kind, StdioStream> + 'static,
{
    fn new(protocol: P) -> Self {
        StdioServer {
            protocol,
            _marker: PhantomData,
        }
    }

    fn serve<S>(self, new_service: S)
    where
        S: NewService + 'static,
        P::ServiceRequest: 'static,
        P::ServiceResponse: 'static,
        P::ServiceError: 'static,
        S::Request: From<P::ServiceRequest>,
        S::Response: Into<P::ServiceResponse>,
        S::Error: Into<P::ServiceError>,
    {
        let new_service = Arc::new(new_service);
        self.with_handle(move |_| new_service.clone())
    }

    fn with_handle<F, S>(self, new_service: F)
    where
        F: Fn(&Handle) -> S + 'static,
        S: NewService + 'static,
        P::ServiceRequest: 'static,
        P::ServiceResponse: 'static,
        P::ServiceError: 'static,
        S::Request: From<P::ServiceRequest>,
        S::Response: Into<P::ServiceResponse>,
        S::Error: Into<P::ServiceError>,
    {
        fn change_types<A, B, C, D>(r: Result<A, B>) -> Result<C, D>
        where
            A: Into<C>,
            B: Into<D>,
        {
            match r {
                Ok(e) => Ok(e.into()),
                Err(e) => Err(e.into()),
            }
        }

        struct WrapService<S, Request, Response, Error> {
            inner: S,
            _marker: PhantomData<fn() -> (Request, Response, Error)>,
        }

        // TODO: avoid to use BoxFuture
        impl<S, Request, Response, Error> Service for WrapService<S, Request, Response, Error>
        where
            S: Service,
            Request: 'static,
            Response: 'static,
            Error: 'static,
            S::Request: From<Request>,
            S::Response: Into<Response>,
            S::Error: Into<Error>,
            S::Future: 'static
        {
            type Request = Request;
            type Response = Response;
            type Error = Error;
            type Future = Then<
                S::Future,
                Result<Response, Error>,
                fn(Result<S::Response,S::Error>) -> Result<Response, Error>
            >;

            fn call(&self, req: Self::Request) -> Self::Future {
               self.inner
                    .call(req.into())
                    .then(change_types)
            }
        }

        let StdioServer { protocol, .. } = self;
        let (stream, rx_finish) = StdioStream::new(32);

        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let new_service = new_service(&handle);
        let service = new_service.new_service().unwrap();
        protocol.bind_server(
            &handle,
            stream,
            WrapService {
                inner: service,
                _marker: PhantomData,
            },
        );

        core.run(rx_finish).unwrap();
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
    let server = StdioServer::new(LineProto);
    server.serve(|| Ok(Echo));
}
