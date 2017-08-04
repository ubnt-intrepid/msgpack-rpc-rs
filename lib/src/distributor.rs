use futures::{Future, Stream, Sink};
use futures::sync::mpsc::{self, UnboundedSender, UnboundedReceiver};
use tokio_core::reactor::Handle;
use super::message::{Message, Request, Response, Notification};


/// A message distributor to each channel.
pub struct Distributor<T, U>
where
    T: Stream<Item = Message> + 'static,
    U: Sink<SinkItem = Message> + 'static,
{
    stream: T,
    sink: U,
    d_tx0: UnboundedSender<(u64, Request)>,
    d_tx1: UnboundedSender<(u64, Response)>,
    d_tx2: UnboundedSender<Notification>,
    m_rx0: UnboundedReceiver<(u64, Request)>,
    m_rx1: UnboundedReceiver<(u64, Response)>,
    m_rx2: UnboundedReceiver<Notification>,
}

impl<T, U> Distributor<T, U>
where
    T: Stream<Item = Message> + 'static,
    U: Sink<SinkItem = Message> + 'static,
{
    pub fn launch(self, handle: &Handle) {
        let Distributor {
            stream,
            sink,
            d_tx0,
            d_tx1,
            d_tx2,
            m_rx0,
            m_rx1,
            m_rx2,
        } = self;

        handle.spawn(stream.map_err(|_| ()).for_each(move |msg| match msg {
            Message::Request(_, _) => do_send(&d_tx0, msg.into()),
            Message::Response(_, _) => do_send(&d_tx1, msg.into()),
            Message::Notification(_) => do_send(&d_tx2, msg.into()),
        }));

        handle.spawn(
            sink.sink_map_err(|_| ())
                .send_all(m_rx0.map(Into::into).select(m_rx1.map(Into::into)).select(
                    m_rx2.map(Into::into),
                ))
                .map(|_| ()),
        );
    }
}

pub fn distributor<T, U>(
    stream: T,
    sink: U,
) -> (Distributor<T, U>,
      (UnboundedReceiver<(u64, Request)>,
       UnboundedReceiver<(u64, Response)>,
       UnboundedReceiver<Notification>),
      (UnboundedSender<(u64, Request)>,
       UnboundedSender<(u64, Response)>,
       UnboundedSender<Notification>))
where
    T: Stream<Item = Message> + 'static,
    U: Sink<SinkItem = Message> + 'static,
{
    let (d_tx0, d_rx0) = mpsc::unbounded();
    let (d_tx1, d_rx1) = mpsc::unbounded();
    let (d_tx2, d_rx2) = mpsc::unbounded();
    let (m_tx0, m_rx0) = mpsc::unbounded();
    let (m_tx1, m_rx1) = mpsc::unbounded();
    let (m_tx2, m_rx2) = mpsc::unbounded();
    let distributor = Distributor {
        stream,
        sink,
        d_tx0,
        d_tx1,
        d_tx2,
        m_rx0,
        m_rx1,
        m_rx2,
    };
    (distributor, (d_rx0, d_rx1, d_rx2), (m_tx0, m_tx1, m_tx2))
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
