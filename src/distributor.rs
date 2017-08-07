use std::collections::VecDeque;
use futures::{Future, Stream, Sink, Poll, Async, AsyncSink};
use futures::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use futures::sync::oneshot;
use super::message::{EncoderMessage, DecoderMessage, Request, Response, Notification};


pub(crate) struct Demux<T: Stream<Item = DecoderMessage>> {
    stream: Option<T>,
    buffer: Option<DecoderMessage>,
    tx0: UnboundedSender<(u64, Request)>,
    tx1: UnboundedSender<(u64, Response)>,
    tx2: UnboundedSender<Notification>,
}

impl<T: Stream<Item = DecoderMessage>> Demux<T> {
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

    fn try_start_send(&mut self, item: DecoderMessage) -> Poll<(), ()> {
        match item {
            DecoderMessage::Request(id, req) => {
                if let AsyncSink::NotReady((id, req)) =
                    self.tx0.start_send((id, req)).map_err(|_| ())?
                {
                    self.buffer = Some(DecoderMessage::Request(id, req));
                    return Ok(Async::NotReady);
                }
            }
            DecoderMessage::Response(id, res) => {
                if let AsyncSink::NotReady((id, res)) =
                    self.tx1.start_send((id, res)).map_err(|_| ())?
                {
                    self.buffer = Some(DecoderMessage::Response(id, res));
                    return Ok(Async::NotReady);
                }
            }
            DecoderMessage::Notification(not) => {
                if let AsyncSink::NotReady(not) = self.tx2.start_send(not).map_err(|_| ())? {
                    self.buffer = Some(DecoderMessage::Notification(not));
                    return Ok(Async::NotReady);
                }
            }
        }
        Ok(Async::Ready(()))
    }
}

impl<T: Stream<Item = DecoderMessage>> Future for Demux<T> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        if let Some(item) = self.buffer.take() {
            try_ready!(self.try_start_send(item))
        }

        loop {
            match self.stream_mut().poll().map_err(|_| ())? {
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



pub(crate) struct Mux<U: Sink<SinkItem = EncoderMessage>> {
    sink: U,
    buffer: VecDeque<EncoderMessage>,
    rx0: UnboundedReceiver<(u64, Request)>,
    rx1: UnboundedReceiver<(u64, Response)>,
    rx2: UnboundedReceiver<(Notification, oneshot::Sender<()>)>,
}

impl<U: Sink<SinkItem = EncoderMessage>> Mux<U> {
    pub(crate) fn new(
        sink: U,
        rx0: UnboundedReceiver<(u64, Request)>,
        rx1: UnboundedReceiver<(u64, Response)>,
        rx2: UnboundedReceiver<(Notification, oneshot::Sender<()>)>,
    ) -> Self {
        Mux {
            sink,
            buffer: Default::default(),
            rx0,
            rx1,
            rx2,
        }
    }

    fn try_recv(&mut self) -> Poll<bool, ()> {
        let mut count = 0;
        let done0 = match self.rx0.poll()? {
            Async::Ready(Some((id, req))) => {
                self.buffer.push_back(EncoderMessage::Request(id, req));
                count += 1;
                false
            }
            Async::Ready(None) => true,
            Async::NotReady => false,
        };
        let done1 = match self.rx1.poll()? {
            Async::Ready(Some((id, res))) => {
                self.buffer.push_back(EncoderMessage::Response(id, res));
                count += 1;
                false
            }
            Async::Ready(None) => true,
            Async::NotReady => false,
        };
        let done2 = match self.rx2.poll()? {
            Async::Ready(Some((not, sender))) => {
                self.buffer.push_back(
                    EncoderMessage::Notification(not, sender),
                );
                count += 1;
                false
            }
            Async::Ready(None) => true,
            Async::NotReady => false,
        };

        if done0 && done1 && done2 {
            Ok(Async::Ready(true))
        } else if count > 0 {
            Ok(Async::Ready(false))
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

impl<U: Sink<SinkItem = EncoderMessage>> Future for Mux<U> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        try_ready!(self.start_send());
        debug_assert!(self.buffer.len() == 0);
        loop {
            match self.try_recv()? {
                Async::Ready(true) => {
                    try_ready!(self.sink.close().map_err(|_| ()));
                    return Ok(Async::Ready(()));
                }
                Async::Ready(false) => try_ready!(self.start_send()),
                Async::NotReady => {
                    try_ready!(self.sink.poll_complete().map_err(|_| ()));
                    return Ok(Async::NotReady);
                }
            }
        }
    }
}
