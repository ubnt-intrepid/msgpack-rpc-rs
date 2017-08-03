use futures::{Future, Stream, Sink};
use futures::sync::mpsc::{self, Sender, Receiver};
use tokio_core::reactor::Handle;


pub fn demux3<S, T0, T1, T2>(
    handle: &Handle,
    stream: S,
) -> (Receiver<T0>, Receiver<T1>, Receiver<T2>)
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

    handle.spawn(stream.for_each(move |msg| match msg.to_demux_id() {
        0 => do_send(&tx0, msg.into()),
        1 => do_send(&tx1, msg.into()),
        2 => do_send(&tx2, msg.into()),
        _ => unreachable!(),
    }));

    (rx0, rx1, rx2)
}


pub fn mux3<S, T0, T1, T2>(handle: &Handle, sink: S) -> (Sender<T0>, Sender<T1>, Sender<T2>)
where
    S: Sink<SinkError = ()> + 'static,
    S::SinkItem: 'static,
    T0: Into<S::SinkItem> + 'static,
    T1: Into<S::SinkItem> + 'static,
    T2: Into<S::SinkItem> + 'static,
{
    // TODO: choose appropriate buffer length.
    let (tx0, rx0) = mpsc::channel(1);
    let (tx1, rx1) = mpsc::channel(1);
    let (tx2, rx2) = mpsc::channel(1);

    handle.spawn(
        sink.send_all(rx0.map(Into::into).select(rx1.map(Into::into)).select(
            rx2.map(
                Into::into,
            ),
        )).map(|_| ()),
    );

    (tx0, tx1, tx2)
}



pub trait ToDemuxId {
    fn to_demux_id(&self) -> u64;
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
