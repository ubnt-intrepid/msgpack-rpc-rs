use std::collections::VecDeque;
use futures::{Future, Stream, Sink, Poll, Async, AsyncSink};
use futures::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use super::message::{Message, Request, Response, Notification};


pub(crate) struct Demux<T: Stream<Item = Message>> {
    stream: Option<T>,
    buffer: Option<Message>,
    tx0: UnboundedSender<(u64, Request)>,
    tx1: UnboundedSender<(u64, Response)>,
    tx2: UnboundedSender<Notification>,
}

impl<T: Stream<Item = Message>> Demux<T> {
    pub(crate) fn new(
        stream: T,
        tx0: UnboundedSender<(u64, Request)>,
        tx1: UnboundedSender<(u64, Response)>,
        tx2: UnboundedSender<Notification>,
    ) -> Self {
        Demux {
            stream: Some(stream),
            buffer: None,
            tx0,
            tx1,
            tx2,
        }
    }

    fn stream_mut(&mut self) -> &mut T {
        self.stream.as_mut().take().unwrap()
    }

    fn try_start_send(&mut self, item: Message) -> Poll<(), ()> {
        match item {
            Message::Request(id, req) => Self::do_send(&mut self.tx0, (id, req), &mut self.buffer),
            Message::Response(id, res) => Self::do_send(&mut self.tx1, (id, res), &mut self.buffer),
            Message::Notification(not) => Self::do_send(&mut self.tx2, not, &mut self.buffer),
        }
    }

    fn do_send<U>(tx: &mut U, item: U::SinkItem, buffer: &mut Option<T::Item>) -> Poll<(), ()>
    where
        U: Sink,
        U::SinkItem: Into<T::Item>,
    {
        if let AsyncSink::NotReady(item) = tx.start_send(item).map_err(|_| ())? {
            *buffer = Some(item.into());
            return Ok(Async::NotReady);
        }
        Ok(Async::Ready(()))
    }
}

impl<T: Stream<Item = Message>> Future for Demux<T> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        if let Some(item) = self.buffer.take() {
            try_ready!(self.try_start_send(item))
        }

        loop {
            match try!(self.stream_mut().poll().map_err(|_| ())) {
                Async::Ready(Some(item)) => try_ready!(self.try_start_send(item)),
                Async::Ready(None) => {
                    try_ready!(self.tx0.close().map_err(|_| ()));
                    try_ready!(self.tx1.close().map_err(|_| ()));
                    try_ready!(self.tx2.close().map_err(|_| ()));
                    self.stream = None;
                    return Ok(Async::Ready(()));
                }
                Async::NotReady => {
                    try_ready!(self.tx0.poll_complete().map_err(|_| ()));
                    try_ready!(self.tx1.poll_complete().map_err(|_| ()));
                    try_ready!(self.tx2.poll_complete().map_err(|_| ()));
                    return Ok(Async::NotReady);
                }
            }
        }
    }
}


pub(crate) struct Mux<U: Sink<SinkItem = Message>> {
    sink: U,
    buffer: VecDeque<Message>,
    rx0: UnboundedReceiver<(u64, Request)>,
    rx1: UnboundedReceiver<(u64, Response)>,
    rx2: UnboundedReceiver<Notification>,
}

impl<U: Sink<SinkItem = Message>> Mux<U> {
    pub(crate) fn new(
        sink: U,
        rx0: UnboundedReceiver<(u64, Request)>,
        rx1: UnboundedReceiver<(u64, Response)>,
        rx2: UnboundedReceiver<Notification>,
    ) -> Self {
        Mux {
            sink,
            buffer: Default::default(),
            rx0,
            rx1,
            rx2,
        }
    }

    fn try_recv(&mut self) -> Poll<Option<usize>, ()> {
        let mut count = 0;
        let done0 = do_recv(&mut self.rx0, &mut self.buffer, &mut count)?;
        let done1 = do_recv(&mut self.rx1, &mut self.buffer, &mut count)?;
        let done2 = do_recv(&mut self.rx2, &mut self.buffer, &mut count)?;

        if done0 && done1 && done2 {
            Ok(Async::Ready(None))
        } else if count > 0 {
            Ok(Async::Ready(Some(count)))
        } else {
            Ok(Async::NotReady)
        }
    }

    fn start_send(&mut self) -> Poll<(), ()> {
        if let Some(item) = self.buffer.pop_front() {
            if let AsyncSink::NotReady(item) = self.sink.start_send(item).map_err(|_| ())? {
                self.buffer.push_front(item);
                return Ok(Async::NotReady);
            } else if self.buffer.len() > 0 {
                return Ok(Async::NotReady);
            }
        }
        Ok(Async::Ready(()))
    }
}

impl<U: Sink<SinkItem = Message>> Future for Mux<U> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        try_ready!(self.start_send());
        debug_assert!(self.buffer.len() == 0);
        loop {
            match try!(self.try_recv()) {
                Async::Ready(Some(_len)) => try_ready!(self.start_send()),
                Async::Ready(None) => {
                    try_ready!(self.sink.close().map_err(|_| ()));
                    return Ok(Async::Ready(()));
                }
                Async::NotReady => {
                    try_ready!(self.sink.poll_complete().map_err(|_| ()));
                    return Ok(Async::NotReady);
                }
            }
        }
    }
}


fn do_recv<T>(rx: &mut T, buffer: &mut VecDeque<Message>, count: &mut usize) -> Result<bool, ()>
where
    T: Stream<Error = ()>,
    T::Item: Into<Message>,
{
    match rx.poll()? {
        Async::Ready(Some(item)) => {
            buffer.push_back(item.into());
            *count += 1;
            Ok(false)
        }
        Async::Ready(None) => Ok(true),
        Async::NotReady => Ok(false),
    }
}
