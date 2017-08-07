//!
//! Protocol definitions
//!

use std::io;
use bytes::{BufMut, BytesMut};
use tokio_io::codec::{Encoder, Decoder};
use super::message::{self, DecodeError};


/// A codec for `Message`.
pub struct Codec;

impl Encoder for Codec {
    type Item = message::EncoderMessage;
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
        msg.into_writer(&mut buf.writer())
    }
}

impl Decoder for Codec {
    type Item = message::DecoderMessage;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        let (res, pos);
        {
            let mut buf = io::Cursor::new(&src);
            res = loop {
                match message::DecoderMessage::from_reader(&mut buf) {
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
