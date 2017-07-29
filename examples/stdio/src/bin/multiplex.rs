extern crate neovim;
extern crate bytes;
extern crate futures;
extern crate tokio_io;
extern crate tokio_proto;
extern crate tokio_service;

use neovim::stdio::StdioServer;
use std::io;
use bytes::{BytesMut, Buf, BufMut, IntoBuf, BigEndian};
use futures::{Future, BoxFuture, IntoFuture};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{Framed, Decoder, Encoder};
use tokio_proto::multiplex::{RequestId, ServerProto};
use tokio_service::Service;


struct LineCodec;

impl Decoder for LineCodec {
    type Item = (RequestId, String);
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        if buf.len() < 5 {
            return Ok(None);
        }
        let newline = buf[4..].iter().position(|&b| b == b'\n');
        if let Some(n) = newline {
            let mut line = buf.split_to(n + 4);
            buf.split_to(1);
            let id = line.split_to(4).into_buf().get_u32::<BigEndian>();
            let s = std::str::from_utf8(&line[..]).map_err(|_| {
                io::Error::new(io::ErrorKind::Other, "invalid UTF-8")
            })?;
            Ok(Some((id as RequestId, s.to_string())))
        } else {
            Ok(None)
        }
    }
}

impl Encoder for LineCodec {
    type Item = (RequestId, String);
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
        let (id, msg) = msg;
        buf.put_u32::<BigEndian>(id as u32);
        buf.put(msg.as_bytes());
        buf.put("\n");
        Ok(())
    }
}


struct LineProto;

impl<T: AsyncRead + AsyncWrite + 'static> ServerProto<T> for LineProto {
    type Request = String;
    type Response = String;
    type Transport = Framed<T, LineCodec>;
    type BindTransport = io::Result<Self::Transport>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(LineCodec))
    }
}


struct Echo;

impl Service for Echo {
    type Request = String;
    type Response = String;
    type Error = io::Error;
    type Future = BoxFuture<Self::Response, Self::Error>;
    fn call(&self, req: Self::Request) -> Self::Future {
        Ok(format!("Received: {:?}", req)).into_future().boxed()
    }
}


fn main() {
    let server = StdioServer::new(LineProto, 1);
    server.serve(|| Ok(Echo));
}
