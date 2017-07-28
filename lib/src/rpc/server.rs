use std::io;
use bytes::{BufMut, BytesMut};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{Framed, Encoder, Decoder};
use tokio_proto::multiplex::ServerProto;

use super::{Request, Response};
use super::errors::DecodeError;


///
pub struct Codec;

impl Encoder for Codec {
    type Item = (u64, Response);
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
        let (id, res) = msg;
        res.to_writer(id, &mut buf.writer())
    }
}

impl Decoder for Codec {
    type Item = (u64, Request);
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        let (req, pos);
        {
            let mut buf = io::Cursor::new(&src);
            req = loop {
                match Request::from_reader(&mut buf) {
                    Ok(message) => break Ok(Some(message)),
                    Err(DecodeError::Truncated) => return Ok(None),
                    Err(DecodeError::Invalid) => continue,
                    Err(DecodeError::Unknown(err)) => break Err(err),
                }
            };
            pos = buf.position() as usize;
        }
        src.split_to(pos);
        req
    }
}

pub struct Proto;

impl<T: AsyncRead + AsyncWrite + 'static> ServerProto<T> for Proto {
    type Request = Request;
    type Response = Response;

    type Transport = Framed<T, Codec>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(Codec))
    }
}


// TODO: implement Server
