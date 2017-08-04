use futures::{Future, Stream, Sink};
use futures::sync::mpsc::{self, Sender, Receiver};
use tokio_core::reactor::Handle;
use super::message::{Message, Request, Response, Notification};


pub struct Distributor {
    task_demux: Box<Future<Item = (), Error = ()>>,
    task_mux: Box<Future<Item = (), Error = ()>>,
}

impl Distributor {
    pub fn launch(self, handle: &Handle) {
        handle.spawn(self.task_demux);
        handle.spawn(self.task_mux);
    }
}

pub fn distributor<T, U>(
    stream: T,
    sink: U,
) -> (Distributor,
      (Receiver<(u64, Request)>, Receiver<(u64, Response)>, Receiver<Notification>),
      (Sender<(u64, Request)>, Sender<(u64, Response)>, Sender<Notification>))
where
    T: Stream<Item = Message> + 'static,
    U: Sink<SinkItem = Message> + 'static,
{
    let (demux_out, task_demux) = {
        // TODO: choose appropriate buffer length.
        let (tx0, rx0) = mpsc::channel(1);
        let (tx1, rx1) = mpsc::channel(1);
        let (tx2, rx2) = mpsc::channel(1);
        let task = stream.map_err(|_| ()).for_each(move |msg| match msg {
            Message::Request(_, _) => do_send(&tx0, msg.into()),
            Message::Response(_, _) => do_send(&tx1, msg.into()),
            Message::Notification(_) => do_send(&tx2, msg.into()),
        });
        ((rx0, rx1, rx2), Box::new(task))
    };

    let (mux_in, task_mux) = {
        // TODO: choose appropriate buffer length.
        let (tx0, rx0) = mpsc::channel(1);
        let (tx1, rx1) = mpsc::channel(1);
        let (tx2, rx2) = mpsc::channel(1);
        let task = sink.sink_map_err(|_| ())
            .send_all(rx0.map(Into::into).select(rx1.map(Into::into)).select(
                rx2.map(
                    Into::into,
                ),
            ))
            .map(|_| ());
        ((tx0, tx1, tx2), Box::new(task))
    };

    let distributor = Distributor {
        task_demux,
        task_mux,
    };
    (distributor, demux_out, mux_in)
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