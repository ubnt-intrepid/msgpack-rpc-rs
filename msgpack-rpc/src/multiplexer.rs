use futures::{Future, Stream, Sink};
use futures::sync::mpsc::{self, Sender, Receiver};
use super::message::Message;


pub trait ToDemuxId {
    fn to_demux_id(&self) -> u64;
}

impl ToDemuxId for Message {
    fn to_demux_id(&self) -> u64 {
        match *self {
            Message::Request(_, _) => 0,
            Message::Response(_, _) => 1,
            Message::Notification(_) => 2,
        }
    }
}


#[cfg_attr(rustfmt, rustfmt_skip)]
pub fn demux3<S, T0, T1, T2>(stream: S) -> (
    (Receiver<T0>, Receiver<T1>, Receiver<T2>),
    Box<Future<Item = (), Error = ()>>,
)
where
    S: Stream< Error = ()> + 'static,
    S::Item: ToDemuxId + Into<T0> + Into<T1> + Into<T2>+ 'static,
    T0: 'static,
    T1: 'static,
    T2: 'static,
{
    // TODO: choose appropriate buffer length.
    let (tx0, rx0) = mpsc::channel(1);
    let (tx1, rx1) = mpsc::channel(1);
    let (tx2, rx2) = mpsc::channel(1);

    let task = stream.for_each(move |msg| {
        match msg.to_demux_id() {
            0 => do_send(&tx0, msg.into()),
            1 => do_send(&tx1, msg.into()),
            2 => do_send(&tx2, msg.into()),
            _ => unreachable!(),
        }
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
pub fn mux3<T, T0, T1, T2>()
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
