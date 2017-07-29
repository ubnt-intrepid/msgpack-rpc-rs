extern crate neovim;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_proto;
extern crate tokio_service;
extern crate futures;
extern crate bytes;

use neovim::nvim::StdioStream;

use std::io;
use std::thread;

use bytes::{BytesMut, Buf, BufMut, BigEndian, IntoBuf};
use futures::{Future, IntoFuture};
use futures::future::BoxFuture;
use futures::sync::oneshot;
use tokio_core::reactor::Core;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{Encoder, Decoder, Framed};
use tokio_proto::BindServer;
use tokio_proto::multiplex::{ServerProto, RequestId};
use tokio_service::Service;

struct LineCodec;
impl Decoder for LineCodec {
    type Item = (RequestId, String);
    type Error = io::Error;
    fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        if buf.len() < 5 {
            return Ok(None);
        }
        let newline = buf[4..].iter().position(|b| *b == b'\n');
        if let Some(n) = newline {
            let mut line = buf.split_to(n + 4);
            buf.split_to(1);
            let id = line.split_to(4).into_buf().get_u32::<BigEndian>();
            return match std::str::from_utf8(&line[..]) {
                Ok(s) => Ok(Some((id as RequestId, s.to_string()))),
                Err(_) => Err(io::Error::new(io::ErrorKind::Other, "invalid string")),
            };
        }
        Ok(None)
    }
}
impl Encoder for LineCodec {
    type Item = (RequestId, String);
    type Error = io::Error;
    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
        let len = 4 + buf.len() + 1;
        buf.reserve(len);
        let (request_id, msg) = msg;
        buf.put_u32::<BigEndian>(request_id as u32);
        buf.put_slice(msg.as_bytes());
        buf.put_u8(b'\n');
        Ok(())
    }
}

struct LineProto;
impl<T: AsyncRead + AsyncWrite + 'static> ServerProto<T> for LineProto {
    type Request = String;
    type Response = String;
    type Transport = Framed<T, LineCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;
    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(LineCodec))
    }
}

struct LineService;
impl Service for LineService {
    type Request = String;
    type Response = String;
    type Error = io::Error;
    type Future = BoxFuture<Self::Response, Self::Error>;
    fn call(&self, req: Self::Request) -> Self::Future {
        Ok(format!("Received: {:?}", req)).into_future().boxed()
    }
}

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let stream = StdioStream::new();
    LineProto.bind_server(&handle, stream, LineService);

    let (tx, rx) = oneshot::channel();
    let loop_fn = |_tx: oneshot::Sender<()>| loop {};
    thread::spawn(move || loop_fn(tx));

    core.run(rx).unwrap();
}
