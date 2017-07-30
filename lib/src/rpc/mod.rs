//!
//! An implementation of Msgpack-RPC, based on tokio-proto and rmp.
//!
//! Currently, notification messages are not supported.
//!

mod message;
mod errors;
pub mod client;
pub mod server;

pub use rmpv::Value;
pub use self::message::{Request, Response};
pub use self::errors::DecodeError;
