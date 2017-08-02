use futures::{Future, Stream, Sink};
use futures::sync::mpsc::{self, Sender, Receiver};
use super::message::{Message, Request, Response, Notification};


#[cfg_attr(rustfmt, rustfmt_skip)]
pub fn demux<S>(stream: S) -> (
    (Receiver<(u64, Request)>, Receiver<(u64, Response)>, Receiver<Notification>),
    Box<Future<Item = (), Error = ()>>,
)
where
    S: Stream<Item = Message, Error = ()> + 'static,
{
    // TODO: choose appropriate buffer length.
    let (tx0, rx0) = mpsc::channel(1);
    let (tx1, rx1) = mpsc::channel(1);
    let (tx2, rx2) = mpsc::channel(1);

    let task = stream.for_each(move |msg| match msg {
        Message::Request(id, req) => do_send(&tx0, (id, req)),
        Message::Response(id, res) => do_send(&tx1, (id, res)),
        Message::Notification(not) => do_send(&tx2, not),
    });

    ((rx0, rx1, rx2), Box::new(task))
}

fn do_send<S>(sink: &S, item: S::SinkItem) -> Box<Future<Item = (), Error = ()>>
where
    S: Sink + Clone + 'static,
{
    Box::new(sink.clone().send(item).then(|res| match res {
        Ok(_) => Ok(()),
        Err(_) => Err(()),
    }))
}


/// Create a multiplexer associated with given sink.
///
/// The return value is output channels and the future of background task.
/// You should spawn the task to process
pub fn mux<T, T0, T1, T2>()
    -> ((Sender<T0>, Sender<T1>, Sender<T2>), Box<Stream<Item = T, Error = ()>>)
where
    T: 'static,
    T0: Into<T> + 'static,
    T1: Into<T> + 'static,
    T2: Into<T> + 'static,
{
    // TODO: choose appropriate buffer length.
    let (tx0, rx0) = mpsc::channel(1);
    let (tx1, rx1) = mpsc::channel(1);
    let (tx2, rx2) = mpsc::channel(1);

    let stream = (rx0.map(Into::into)).select(rx1.map(Into::into)).select(
        rx2.map(
            Into::into,
        ),
    );

    ((tx0, tx1, tx2), Box::new(stream))
}
