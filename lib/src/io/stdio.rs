use std::cmp;
use std::io::{self, Read, Write, Stdout};
use std::thread;

use bytes::BytesMut;
use futures::{Future, Stream, Sink, Poll, Async};
use futures::sync::mpsc;
use tokio_io::{AsyncRead, AsyncWrite};


pub struct Stdin {
    rx_stdin: mpsc::UnboundedReceiver<io::Result<Vec<u8>>>,
    buffer: BytesMut,
    eof: bool,
}

impl Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.eof {
            return Ok(0);
        }

        if self.buffer.len() > 0 {
            let len = cmp::min(self.buffer.len(), buf.len());
            buf[0..len].copy_from_slice(&self.buffer[0..len]);
            self.buffer.split_to(len);
            return Ok(len);
        }

        debug_assert_eq!(self.buffer.len(), 0);

        match self.rx_stdin.poll() {
            Ok(Async::Ready(Some(Ok(bytes)))) => {
                let len = cmp::min(bytes.len(), buf.len());
                buf[0..len].copy_from_slice(&bytes[0..len]);
                if len < bytes.len() {
                    self.buffer.extend_from_slice(&bytes[len..]);
                }
                Ok(len)
            }
            Ok(Async::Ready(Some(Err(err)))) => Err(err),
            Ok(Async::Ready(None)) => {
                self.eof = true;
                Ok(0)
            }
            Ok(Async::NotReady) => Err(io::Error::new(io::ErrorKind::WouldBlock, "Not ready")),
            Err(()) => Err(io::Error::new(io::ErrorKind::Other, "broken pipe")),
        }
    }
}

impl AsyncRead for Stdin {}

pub fn stdin(chunk_size: usize) -> Stdin {
    assert!(chunk_size > 0);

    let (mut tx_stdin, rx_stdin) = mpsc::unbounded();
    thread::spawn(move || {
        let stdin = io::stdin();
        let mut locked_stdin = stdin.lock();
        loop {
            let mut bytes = vec![0u8; chunk_size];
            match locked_stdin.read(&mut bytes) {
                Ok(n_bytes) => {
                    bytes.truncate(n_bytes);
                    match tx_stdin.send(Ok(bytes)).wait() {
                        Ok(t) => tx_stdin = t,
                        Err(_) => break,
                    }
                }
                Err(err) => {
                    let _ = tx_stdin.send(Err(err)).wait();
                    break;
                }
            }
        }
    });

    Stdin {
        rx_stdin,
        buffer: BytesMut::new(),
        eof: false,
    }
}


pub struct StdioStream {
    stdin: Stdin,
    stdout: Stdout,
}

impl StdioStream {
    pub fn new(chunk_size: usize) -> Self {
        let stdin = stdin(chunk_size);
        let stdout = io::stdout();
        StdioStream { stdin, stdout }
    }
}

impl Read for StdioStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stdin.read(buf)
    }
}

impl AsyncRead for StdioStream {}

impl Write for StdioStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stdout.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}

impl AsyncWrite for StdioStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        Ok(Async::Ready(()))
    }
}
