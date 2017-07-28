use std::io;


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
