use std::io;
use bytes::{BufMut, BytesMut};
use futures::{Stream, Sink};
use tokio_io::codec::{Encoder, Decoder};
use super::message::{Message, Request, Response, DecodeError};


/// A codec for `Message`.
pub struct Codec;

impl Encoder for Codec {
    type Item = Message;
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
        msg.to_writer(&mut buf.writer())
    }
}

impl Decoder for Codec {
    type Item = Message;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        let (res, pos);
        {
            let mut buf = io::Cursor::new(&src);
            res = loop {
                match Message::from_reader(&mut buf) {
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


/// A protocol definition of Msgpack-RPC.
///
/// Note that this protocol is only available for (framed) pairs of a stream/sink,
/// because it will share same I/O stream both of client/server use.
/// `tokio_proto` does not support such situation.
pub struct Proto;

impl<T> ::tokio_proto::multiplex::ClientProto<T> for Proto
where
    T: 'static
        + Stream<Item = (u64, Response), Error = io::Error>
        + Sink<SinkItem = (u64, Request), SinkError = io::Error>,
{
    type Request = Request;
    type Response = Response;
    type Transport = T;
    type BindTransport = io::Result<Self::Transport>;
    fn bind_transport(&self, transport: T) -> Self::BindTransport {
        Ok(transport)
    }
}

impl<T> ::tokio_proto::multiplex::ServerProto<T> for Proto
where
    T: 'static
        + Stream<Item = (u64, Request), Error = io::Error>
        + Sink<SinkItem = (u64, Response), SinkError = io::Error>,
{
    type Request = Request;
    type Response = Response;
    type Transport = T;
    type BindTransport = io::Result<Self::Transport>;
    fn bind_transport(&self, transport: T) -> Self::BindTransport {
        Ok(transport)
    }
}

