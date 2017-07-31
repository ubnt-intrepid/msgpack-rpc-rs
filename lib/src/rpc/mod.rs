//!
//! An implementation of Msgpack-RPC, based on tokio-proto and rmp.
//!
//! Currently, notification messages are not supported.
//!

mod codec;
mod message;
mod errors;
pub mod bidirectional;
pub mod client;
pub mod server;

pub use rmpv::Value;
pub use self::message::{Message, Request, Response, Notification};
pub use self::errors::DecodeError;
