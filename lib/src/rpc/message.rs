use std::io::{self, Read, Write};
use rmpv::{self, Value};
use super::errors::DecodeError;


pub const REQUEST_TYPE: i64 = 0;
pub const RESPONSE_TYPE: i64 = 1;
pub const NOTIFICATION_TYPE: i64 = 2;


#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub params: Vec<Value>,
}

impl Request {
    pub fn new<S: Into<String>>(method: S) -> Self {
        Request {
            method: method.into(),
            params: vec![],
        }
    }

    pub fn param(mut self, param: Value) -> Self {
        self.params.push(param);
        self
    }

    pub fn params(mut self, params: Vec<Value>) -> Self {
        self.params.extend(params);
        self
    }

    pub fn from_reader<R: Read>(r: &mut R) -> Result<(u64, Request), DecodeError> {
        let value = rmpv::decode::read_value(r).map_err(|err| {
            use rmpv::decode::Error::*;
            match err {
                InvalidMarkerRead(err) |
                InvalidDataRead(err) => DecodeError::from(err),
            }
        })?;
        let array = value.as_array().ok_or(DecodeError::Invalid)?;
        if array.len() < 4 {
            return Err(DecodeError::Invalid);
        }
        let msgtype = array[0].as_i64().ok_or(DecodeError::Invalid)?;
        if msgtype != REQUEST_TYPE {
            return Err(DecodeError::Invalid);
        }

        let id = array[1].as_i64().ok_or(DecodeError::Invalid)? as u64;
        let method = array[2].as_str().ok_or(DecodeError::Invalid)?.to_owned();
        let params = array[3].as_array().ok_or(DecodeError::Invalid)?.clone();

        let request = Request { method, params };
        Ok((id as u64, request))
    }

    pub fn to_writer<W: Write>(&self, id: u64, w: &mut W) -> io::Result<()> {
        // TODO: use ValueRef to avoid cloning
        let packet = Value::Array(vec![
            Value::Integer(REQUEST_TYPE.into()),
            Value::Integer(id.into()),
            Value::String(self.method.clone().into()),
            Value::Array(self.params.clone()),
        ]);
        rmpv::encode::write_value(w, &packet).map_err(|err| {
            use rmpv::encode::Error::*;
            match err {
                InvalidMarkerWrite(e) |
                InvalidDataWrite(e) => e,
            }
        })
    }
}

#[derive(Debug)]
pub struct Response(pub Result<Value, Value>);

impl Response {
    pub fn from_reader<R: Read>(r: &mut R) -> Result<(u64, Response), DecodeError> {
        let value = rmpv::decode::read_value(r).map_err(|err| {
            use rmpv::decode::Error::*;
            match err {
                InvalidMarkerRead(err) |
                InvalidDataRead(err) => DecodeError::from(err),
            }
        })?;
        let array = value.as_array().ok_or(DecodeError::Invalid)?;
        if array.len() < 4 {
            return Err(DecodeError::Invalid);
        }
        let msgtype = array[0].as_i64().ok_or(DecodeError::Invalid)?;
        if msgtype != RESPONSE_TYPE {
            return Err(DecodeError::Invalid);
        }

        let id = array[1].as_i64().ok_or(DecodeError::Invalid)?;

        let response = match (&array[2], &array[3]) {
            (val, &Value::Nil) => Response(Ok(val.clone())),
            (&Value::Nil, val) => Response(Err(val.clone())),
            _ => return Err(DecodeError::Invalid),
        };

        Ok((id as u64, response))
    }

    pub fn to_writer<W: Write>(&self, id: u64, w: &mut W) -> io::Result<()> {
        // TODO: use ValueRef to avoid cloning
        let packet = Value::Array(vec![
            Value::Integer(RESPONSE_TYPE.into()),
            Value::Integer(id.into()),
            self.0.as_ref().ok().cloned().unwrap_or(Value::Nil),
            self.0.as_ref().err().cloned().unwrap_or(Value::Nil),
        ]);
        rmpv::encode::write_value(w, &packet).map_err(|err| {
            use rmpv::encode::Error::*;
            match err {
                InvalidMarkerWrite(e) |
                InvalidDataWrite(e) => e,
            }
        })
    }
}
