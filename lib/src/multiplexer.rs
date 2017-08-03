use std::marker::PhantomData;
use futures::{Future, Stream, Sink, Poll};
use futures::stream::{Select, Map};
use futures::sync::mpsc::{self, Sender, Receiver};


pub trait ToDemuxId {
    fn to_demux_id(&self) -> u64;
}


pub struct Demux3<S, T0, T1, T2>
where
    S: Stream<Error = ()>,
    S::Item: Into<T0> + Into<T1> + Into<T2> + ToDemuxId + 'static,
    T0: 'static,
    T1: 'static,
    T2: 'static,
{
    tx0: Sender<T0>,
    tx1: Sender<T1>,
    tx2: Sender<T2>,
    _marker: PhantomData<S>,
}

impl<S, T0, T1, T2> Demux3<S, T0, T1, T2>
where
    S: Stream<Error=()>,
    S::Item: Into<T0> + Into<T1> + Into<T2> + ToDemuxId + 'static,
    T0: 'static,
    T1: 'static,
    T2: 'static,
{
    pub fn new(tx0: Sender<T0>, tx1: Sender<T1>, tx2: Sender<T2>) -> Self {
        Demux3 {
            tx0,
            tx1,
            tx2,
            _marker: PhantomData,
        }
    }

    pub fn send(&self, msg: S::Item) -> Box<Future<Item = (), Error = ()>> {
        match msg.to_demux_id() {
            0 => Self::do_send(&self.tx0, msg.into()),
            1 => Self::do_send(&self.tx1, msg.into()),
            2 => Self::do_send(&self.tx2, msg.into()),
            _ => unreachable!(),
        }
    }

    fn do_send<U>(sink: &U, item: U::SinkItem) -> Box<Future<Item = (), Error = ()>>
    where
        U: Sink + Clone + 'static,
    {
        Box::new(sink.clone().send(item).then(|res| match res {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }))
    }
}

pub fn demux3<S, T0, T1, T2>(
    stream: S,
) -> ((Receiver<T0>, Receiver<T1>, Receiver<T2>), Box<Future<Item = (), Error = ()>>)
where
    S: Stream<Error = ()> + 'static,
    S::Item: ToDemuxId + Into<T0> + Into<T1> + Into<T2> + 'static,
    T0: 'static,
    T1: 'static,
    T2: 'static,
{
    // TODO: choose appropriate buffer length.
    let (tx0, rx0) = mpsc::channel(1);
    let (tx1, rx1) = mpsc::channel(1);
    let (tx2, rx2) = mpsc::channel(1);
    let demux = Demux3::<S, T0, T1, T2>::new(tx0, tx1, tx2);

    let task = stream.for_each(move |msg| demux.send(msg));

    ((rx0, rx1, rx2), Box::new(task))
}



/// A multiplexed stream from 3 inputs.
pub struct Mux3Stream<T, T0, T1, T2> {
    inner: Select<
        Select<Map<Receiver<T0>, fn(T0) -> T>, Map<Receiver<T1>, fn(T1) -> T>>,
        Map<Receiver<T2>, fn(T2) -> T>,
    >,
}

impl<T, T0, T1, T2> Mux3Stream<T, T0, T1, T2>
where
    T: 'static,
    T0: Into<T> + 'static,
    T1: Into<T> + 'static,
    T2: Into<T> + 'static,
{
    fn new(rx0: Receiver<T0>, rx1: Receiver<T1>, rx2: Receiver<T2>) -> Self {
        let inner = (rx0.map(Into::into as fn(T0) -> T))
            .select(rx1.map(Into::into as fn(T1) -> T))
            .select(rx2.map(Into::into as fn(T2) -> T));
        Mux3Stream { inner }
    }
}

impl<T, T0, T1, T2> Stream for Mux3Stream<T, T0, T1, T2> {
    type Item = T;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.inner.poll()
    }
}

/// Create a pair of input channels and mulitiplexed stream associated with the inputs.
pub fn mux3<T, T0, T1, T2>() -> ((Sender<T0>, Sender<T1>, Sender<T2>), Mux3Stream<T, T0, T1, T2>)
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

    let stream = Mux3Stream::new(rx0, rx1, rx2);

    ((tx0, tx1, tx2), stream)
}