use std::cmp;
use std::io::{self, Read, Write, Stdout};
use std::thread;

use bytes::BytesMut;
use futures::{Future, Stream, Sink, Poll, Async};
use futures::sync::{mpsc, oneshot};
use tokio_io::{AsyncRead, AsyncWrite};


pub struct StdioStream {
    rx_stdin: mpsc::Receiver<io::Result<Vec<u8>>>,
    buffer: BytesMut,
    is_finished: bool,
    stdout: Stdout,
}

impl StdioStream {
    pub fn new(chunk_size: usize) -> (Self, oneshot::Receiver<()>) {
        assert!(chunk_size > 0);

        let (tx_stdin, rx_stdin) = mpsc::channel(0);
        let (tx_finish, rx_finish) = oneshot::channel::<()>();
        thread::spawn(move || Self::stdin_loop(tx_stdin, tx_finish, chunk_size));

        let stdout = io::stdout();
        let buffer = BytesMut::new();

        let stream = StdioStream {
            rx_stdin,
            buffer,
            is_finished: false,
            stdout,
        };
        (stream, rx_finish)
    }

    fn stdin_loop(
        mut tx_stdin: mpsc::Sender<io::Result<Vec<u8>>>,
        tx_finish: oneshot::Sender<()>,
        chunk_size: usize,
    ) {
        let stdin = io::stdin();
        let mut locked_stdin = stdin.lock();
        loop {
            let mut bytes = vec![0u8; chunk_size];
            match locked_stdin.read_exact(&mut bytes) {
                Ok(()) => {
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
        let _ = tx_finish.send(());
    }
}

impl Read for StdioStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.is_finished {
            return Ok(0);
        }

        if self.buffer.len() > 0 {
            // read from buffer.
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
                self.is_finished = true;
                Ok(0)
            }
            Ok(Async::NotReady) => Err(io::Error::new(io::ErrorKind::WouldBlock, "Not ready")),
            Err(()) => Err(io::Error::new(io::ErrorKind::Other, "broken pipe")),
        }
    }
}

impl Write for StdioStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stdout.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}

impl AsyncRead for StdioStream {}

impl AsyncWrite for StdioStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        Ok(Async::Ready(()))
    }
}
