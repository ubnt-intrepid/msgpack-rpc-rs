use std::io::{self, Read};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::Duration;
use rpc::{Request, Response};
use tokio_io::AsyncRead;
use tokio_service::NewService;

pub struct Stdin {
    _tx_req: mpsc::Sender<usize>,
    _rx_data: mpsc::Receiver<io::Result<Vec<u8>>>,
}

impl Stdin {
    pub fn new() -> Self {
        let (_tx_req, rx_req) = mpsc::channel();
        let (tx_data, _rx_data) = mpsc::channel();
        thread::spawn(move || {
            let stdin = io::stdin();
            let mut locked_stdin = stdin.lock();
            loop {
                match rx_req.recv() {
                    Ok(size) => {
                        let mut buf = vec![0u8; size];
                        let res = locked_stdin.read(&mut buf).map(|_| buf);
                        let _ = tx_data.send(res);
                    }
                    Err(_) => break,
                }
            }
        });
        Stdin { _tx_req, _rx_data }
    }
}

impl Read for Stdin {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        // MEMO: treat non-blocking
        Ok(0)
    }
}

impl AsyncRead for Stdin {}



pub struct NeoVim {}

impl NeoVim {
    pub fn serve<S>()
    where
        S: NewService<Request = Request, Response = Response, Error = io::Error>,
    {
    }
}
