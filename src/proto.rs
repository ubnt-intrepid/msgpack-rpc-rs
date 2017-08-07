//!
//! Protocol definitions
//!

use std::io;
use bytes::{BufMut, BytesMut};
use futures::{Stream, Sink, Poll, StartSend, Async, AsyncSink};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{Framed, Encoder, Decoder};
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
    T: 'static + AsyncRead + AsyncWrite,
{
    type Request = Request;
    type Response = Response;
    type Transport = __ClientTransport<T>;
    type BindTransport = io::Result<Self::Transport>;
    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(__ClientTransport(io.framed(Codec)))
    }
}

impl<T> ::tokio_proto::multiplex::ServerProto<T> for Proto
where
    T: 'static + AsyncRead + AsyncWrite,
{
    type Request = Request;
    type Response = Response;
    type Transport = __ServerTransport<T>;
    type BindTransport = io::Result<Self::Transport>;
    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(__ServerTransport(io.framed(Codec)))
    }
}


#[doc(hidden)]
pub struct __ClientTransport<T: AsyncRead + AsyncWrite + 'static>(Framed<T, Codec>);

impl<T: AsyncRead + AsyncWrite + 'static> Stream for __ClientTransport<T> {
    type Item = (u64, Response);
    type Error = io::Error;
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match try_ready!(self.0.poll()) {
            Some(Message::Response(id, res)) => Ok(Async::Ready(Some((id, res)))),
            Some(_) => self.poll(),
            None => Ok(Async::Ready(None)),
        }
    }
}

impl<T: AsyncRead + AsyncWrite + 'static> Sink for __ClientTransport<T> {
    type SinkItem = (u64, Request);
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        match self.0.start_send(Message::from(item)) {
            Ok(AsyncSink::Ready) => Ok(AsyncSink::Ready),
            Ok(AsyncSink::NotReady(Message::Request(id, req))) => Ok(
                AsyncSink::NotReady((id, req)),
            ),
            Ok(AsyncSink::NotReady(_)) => unreachable!(),
            Err(e) => Err(e),
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.0.poll_complete()
    }
}

#[doc(hidden)]
pub struct __ServerTransport<T>(Framed<T, Codec>);

impl<T: AsyncRead + AsyncWrite + 'static> Stream for __ServerTransport<T> {
    type Item = (u64, Request);
    type Error = io::Error;
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match try_ready!(self.0.poll()) {
            Some(Message::Request(id, req)) => Ok(Async::Ready(Some((id, req)))),
            Some(_) => self.poll(),
            None => Ok(Async::Ready(None)),
        }
    }
}

impl<T: AsyncRead + AsyncWrite + 'static> Sink for __ServerTransport<T> {
    type SinkItem = (u64, Response);
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        match self.0.start_send(item.into()) {
            Ok(AsyncSink::Ready) => Ok(AsyncSink::Ready),
            Ok(AsyncSink::NotReady(Message::Response(id, res))) => Ok(
                AsyncSink::NotReady((id, res)),
            ),
            Ok(AsyncSink::NotReady(_)) => unreachable!(),
            Err(e) => Err(e),
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.0.poll_complete()
    }
}
