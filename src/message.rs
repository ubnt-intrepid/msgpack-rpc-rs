use std::io::{self, Read, Write};
use futures::sync::oneshot;
use bytes::{BufMut, BytesMut};
use tokio_io::codec::{Encoder, Decoder};
use rmpv::{self, Value};

const REQUEST_TYPE: i64 = 0;
const RESPONSE_TYPE: i64 = 1;
const NOTIFICATION_TYPE: i64 = 2;


/// A codec for `Message`.
pub struct Codec;

impl Encoder for Codec {
    type Item = EncoderMessage;
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
        msg.into_writer(&mut buf.writer())
    }
}

impl Decoder for Codec {
    type Item = DecoderMessage;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        let (res, pos);
        {
            let mut buf = io::Cursor::new(&src);
            res = loop {
                match DecoderMessage::from_reader(&mut buf) {
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


#[derive(Debug)]
pub enum EncoderMessage {
    Request(u64, Request),
    Response(u64, Response),
    Notification(Notification, oneshot::Sender<()>),
}

impl EncoderMessage {
    /// Write a response to an output stream, with given ID.
    pub fn into_writer<W: Write>(self, w: &mut W) -> io::Result<()> {
        let packet = match self {
            EncoderMessage::Request(id, ref req) => req.to_packet(id),
            EncoderMessage::Response(id, ref res) => res.to_packet(id),
            EncoderMessage::Notification(ref not, _) => not.to_packet(),
        };
        write_packet(w, &packet)?;

        if let EncoderMessage::Notification(_, done) = self {
            let _ = done.send(());
        }

        Ok(())
    }
}


#[derive(Debug)]
pub enum DecoderMessage {
    Request(u64, Request),
    Response(u64, Response),
    Notification(Notification),
}

impl DecoderMessage {
    /// Read a request and its ID from an input stream
    pub fn from_reader<R: Read>(r: &mut R) -> Result<Self, DecodeError> {
        let value = next_value(r)?;
        let array = value.as_array().ok_or(DecodeError::Invalid)?;
        match (array.get(0).and_then(|v| v.as_i64()), array.len()) {
            (Some(REQUEST_TYPE), n) if n >= 4 => Request::from_array(&array[1..4]),
            (Some(RESPONSE_TYPE), n) if n >= 4 => Response::from_array(&array[1..4]),
            (Some(NOTIFICATION_TYPE), n) if n >= 3 => Notification::from_array(&array[1..3]),
            _ => Err(DecodeError::Invalid),
        }
    }
}


/// A request message
#[derive(Debug)]
pub struct Request {
    /// The method name
    pub method: String,
    /// Arguments of the method
    pub params: Value,
}

impl Request {
    /// Create an instance of request
    pub fn new<S: Into<String>, P: Into<Value>>(method: S, params: P) -> Self {
        Request {
            method: method.into(),
            params: params.into(),
        }
    }

    fn from_array(array: &[Value]) -> Result<DecoderMessage, DecodeError> {
        match (array[0].as_i64(), array[1].as_str(), array[2].is_array()) {
            (Some(id), Some(method), true) => {
                Ok(DecoderMessage::Request(
                    id as u64,
                    Request {
                        method: method.to_owned(),
                        params: array[2].clone(),
                    },
                ))
            }
            _ => Err(DecodeError::Invalid),
        }
    }

    fn to_packet(&self, id: u64) -> Value {
        Value::Array(vec![
            REQUEST_TYPE.into(),
            id.into(),
            self.method.as_str().into(),
            self.params.clone(),
        ])
    }
}


/// A response message
#[derive(Debug)]
pub struct Response(Result<Value, Value>);

impl<T: Into<Value>, E: Into<Value>> From<Result<T, E>> for Response {
    fn from(res: Result<T, E>) -> Self {
        match res {
            Ok(t) => Response(Ok(t.into())),
            Err(e) => Response(Err(e.into())),
        }
    }
}

impl Response {
    /// Create an instance of response message from success value
    pub fn from_ok<T: Into<Value>>(value: T) -> Self {
        Response(Ok(value.into()))
    }

    /// Create an instance of response message from error value
    pub fn from_err<E: Into<Value>>(value: E) -> Self {
        Response(Err(value.into()))
    }

    pub fn into_inner(self) -> Result<Value, Value> {
        self.0
    }

    fn from_array(array: &[Value]) -> Result<DecoderMessage, DecodeError> {
        match (array[0].as_i64(), &array[1], &array[2]) {
            (Some(id), val, &Value::Nil) => Ok(DecoderMessage::Response(
                id as u64,
                Response(Ok(val.clone())),
            )),
            (Some(id), &Value::Nil, val) => Ok(DecoderMessage::Response(
                id as u64,
                Response(Err(val.clone())),
            )),
            _ => return Err(DecodeError::Invalid),
        }
    }

    fn to_packet(&self, id: u64) -> Value {
        Value::Array(vec![
            RESPONSE_TYPE.into(),
            id.into(),
            self.0.as_ref().err().cloned().unwrap_or(Value::Nil),
            self.0.as_ref().ok().cloned().unwrap_or(Value::Nil),
        ])
    }
}


/// A notification message
#[derive(Debug)]
pub struct Notification {
    /// The method name
    pub method: String,
    /// Arguments of the method
    pub params: Value,
}

impl Notification {
    /// Create an instance of request
    pub fn new<S: Into<String>, P: Into<Value>>(method: S, params: P) -> Self {
        Notification {
            method: method.into(),
            params: params.into(),
        }
    }

    fn from_array(array: &[Value]) -> Result<DecoderMessage, DecodeError> {
        match (array[0].as_str(), array[1].is_array()) {
            (Some(method), true) => Ok(DecoderMessage::Notification(Notification {
                method: method.to_owned(),
                params: array[1].clone(),
            })),
            _ => Err(DecodeError::Invalid),
        }
    }

    fn to_packet(&self) -> Value {
        Value::Array(vec![
            NOTIFICATION_TYPE.into(),
            self.method.as_str().into(),
            self.params.clone().into(),
        ])
    }
}


pub enum DecodeError {
    Truncated,
    Invalid,
    Unknown(io::Error),
}

impl From<io::Error> for DecodeError {
    fn from(err: io::Error) -> Self {
        match err.kind() {
            io::ErrorKind::UnexpectedEof => DecodeError::Truncated,
            io::ErrorKind::Other => {
                if let Some(cause) = err.get_ref().unwrap().cause() {
                    if cause.description() == "type mismatch" {
                        return DecodeError::Invalid;
                    }
                }
                DecodeError::Unknown(err)
            }
            _ => DecodeError::Unknown(err),
        }
    }
}



fn next_value<R: Read>(r: &mut R) -> Result<Value, DecodeError> {
    rmpv::decode::read_value(r).map_err(|err| {
        use rmpv::decode::Error::*;
        match err {
            InvalidMarkerRead(err) |
            InvalidDataRead(err) => DecodeError::from(err),
        }
    })
}

fn write_packet<W: Write>(w: &mut W, packet: &Value) -> io::Result<()> {
    rmpv::encode::write_value(w, packet).map_err(|err| {
        use rmpv::encode::Error::*;
        match err {
            InvalidMarkerWrite(e) |
            InvalidDataWrite(e) => e,
        }
    })
}
