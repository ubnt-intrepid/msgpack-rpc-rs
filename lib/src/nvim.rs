use std::io;
use rpc::{Request, Response};
use tokio_service::NewService;


pub struct NeoVim;

impl NeoVim {
    pub fn serve<S>()
    where
        S: NewService<Request = Request, Response = Response, Error = io::Error>,
    {
    }
}
