use std::io::{self, Read};
use std::sync::mpsc;
use std::thread;
use rpc::{Request, Response};
use tokio_io::AsyncRead;
use tokio_service::NewService;

pub struct Stdin {
    tx_req: mpsc::Sender<usize>,
    rx_data: mpsc::Receiver<io::Result<Vec<u8>>>,
}

impl Stdin {
    pub fn new() -> Self {
        let (tx_req, rx_req) = mpsc::channel();
        let (tx_data, rx_data) = mpsc::channel();
        thread::spawn(move || {
            let stdin = io::stdin();
            let mut locked_stdin = stdin.lock();
            loop {
                match rx_req.recv() {
                    Ok(size) => {
                        let mut buf = vec![0u8; size];
                        let result = match locked_stdin.read(&mut buf) {
                            Ok(len) => {
                                buf.truncate(len);
                                Ok(buf)
                            }
                            Err(err) => Err(err),
                        };
                        let _ = tx_data.send(result);
                    }
                    Err(_) => {
                        let _ =
                            tx_data.send(Err(io::Error::new(io::ErrorKind::Other, "broken pipe")));
                        break;
                    }
                }
            }
        });
        Stdin { tx_req, rx_data }
    }
}

impl Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.tx_req.send(buf.len()).map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "failed to send request length")
        })?;
        let data = self.rx_data.recv().map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "failed to recv data")
        })??;

        buf[0..(data.len())].copy_from_slice(&data);
        Ok(data.len())
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
