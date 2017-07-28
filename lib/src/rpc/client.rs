use std::io;
use std::net::ToSocketAddrs;
use bytes::{BufMut, BytesMut};
use futures::Future;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{Framed, Encoder, Decoder};
use tokio_proto::TcpClient;
use tokio_proto::multiplex::{ClientProto, ClientService};
use tokio_service::Service;

use super::{Request, Response};
use super::errors::DecodeError;


///
pub struct Codec;

impl Encoder for Codec {
    type Item = (u64, Request);
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
        let (id, req) = msg;
        req.to_writer(id, &mut buf.writer())
    }
}

impl Decoder for Codec {
    type Item = (u64, Response);
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        let (res, pos);
        {
            let mut buf = io::Cursor::new(&src);
            res = loop {
                match Response::from_reader(&mut buf) {
                    Ok(message) => break Ok(Some(message)),
                    Err(DecodeError::Truncated) => return Ok(None),
                    Err(DecodeError::Invalid) => continue,
                    Err(DecodeError::Unknown(err)) => break Err(err),
                }
            };
            pos = buf.position() as usize;
        }
        src.split_to(pos);
        res
    }
}

pub struct Proto;

impl<T: AsyncRead + AsyncWrite + 'static> ClientProto<T> for Proto {
    type Request = Request;
    type Response = Response;

    type Transport = Framed<T, Codec>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(Codec))
    }
}



pub struct Client<T: AsyncRead + AsyncWrite + 'static> {
    inner: ClientService<T, Proto>,
}

impl Client<TcpStream> {
    /// Create a new RPC client with given IP address and event handle.
    ///
    /// # Panics
    /// This function will panic if `addr` is not convertible to `SocketAddr`.
    pub fn connect<A: ToSocketAddrs>(
        addr: A,
        handle: &Handle,
    ) -> Box<Future<Item = Self, Error = io::Error>> {
        let addr = addr.to_socket_addrs().unwrap().next().unwrap();
        let client = TcpClient::new(Proto).connect(&addr, handle).map(|inner| {
            Client { inner }
        });
        Box::new(client)
    }
}

impl<T: AsyncRead + AsyncWrite + 'static> Service for Client<T> {
    type Request = Request;
    type Response = Response;
    type Error = io::Error;
    type Future = <ClientService<T, Proto> as Service>::Future;

    fn call(&self, req: Self::Request) -> Self::Future {
        self.inner.call(req)
    }
}
