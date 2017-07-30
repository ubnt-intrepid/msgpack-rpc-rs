use std::io::{self, Read, Write};
use rmpv::{self, Value};
use super::errors::DecodeError;


const REQUEST_TYPE: i64 = 0;
const RESPONSE_TYPE: i64 = 1;
#[allow(dead_code)]
const NOTIFICATION_TYPE: i64 = 2;


/// A request message
#[derive(Debug)]
pub struct Request {
    /// The method name
    pub method: String,
    /// Arguments of the method
    pub params: Vec<Value>,
}

impl Request {
    /// Create an instance of request
    pub fn new<S: Into<String>>(method: S, params: Vec<Value>) -> Self {
        let method = method.into();
        Request { method, params }
    }

    /// Read a request and its ID from an input stream
    pub fn from_reader<R: Read>(r: &mut R) -> Result<(u64, Request), DecodeError> {
        let value = next_value(r)?;
        let array = value.as_array().ok_or(DecodeError::Invalid)?;
        match (array.get(0).and_then(|v| v.as_i64()), array.len()) {
            (Some(REQUEST_TYPE), 4) => {
                let id = array[1].as_i64().ok_or(DecodeError::Invalid)? as u64;
                let method = array[2].as_str().ok_or(DecodeError::Invalid)?.to_owned();
                let params = array[3].as_array().ok_or(DecodeError::Invalid)?.clone();
                let request = Request { method, params };
                Ok((id, request))
            }
            _ => Err(DecodeError::Invalid),
        }
    }

    /// Write a response to an output stream, with given ID.
    pub fn to_writer<W: Write>(&self, id: u64, w: &mut W) -> io::Result<()> {
        let packet = Value::Array(vec![
            Value::Integer(REQUEST_TYPE.into()),
            Value::Integer(id.into()),
            Value::String(self.method.clone().into()),
            Value::Array(self.params.clone()),
        ]);
        write_packet(w, &packet)
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

    /// Read a response and its ID from an input stream
    pub fn from_reader<R: Read>(r: &mut R) -> Result<(u64, Response), DecodeError> {
        let value = next_value(r)?;
        let array = value.as_array().ok_or(DecodeError::Invalid)?;
        match (array.get(0).and_then(|v| v.as_i64()), array.len()) {
            (Some(RESPONSE_TYPE), 3) => {
                let id = array[1].as_i64().ok_or(DecodeError::Invalid)? as u64;
                let response = match (&array[2], &array[3]) {
                    (val, &Value::Nil) => Response(Ok(val.clone())),
                    (&Value::Nil, val) => Response(Err(val.clone())),
                    _ => return Err(DecodeError::Invalid),
                };
                Ok((id, response))
            }
            _ => Err(DecodeError::Invalid),
        }
    }

    /// Write a response to an output stream, with given ID
    pub fn to_writer<W: Write>(&self, id: u64, w: &mut W) -> io::Result<()> {
        // TODO: use ValueRef to avoid cloning
        let packet = Value::Array(vec![
            Value::Integer(RESPONSE_TYPE.into()),
            Value::Integer(id.into()),
            self.0.as_ref().ok().cloned().unwrap_or(Value::Nil),
            self.0.as_ref().err().cloned().unwrap_or(Value::Nil),
        ]);
        write_packet(w, &packet)
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
