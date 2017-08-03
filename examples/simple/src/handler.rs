use msgpack_rpc::{Request, Response, Notification, Service, NotifyService};

use std::io;
use std::time::Duration;
use std::thread;
use futures::Future;
use futures::sync::oneshot;
use futures::future::{ok, FutureResult};
use rmpv::ext::from_value;


fn the_answer() -> Result<u64, String> {
    Ok(42)
}


#[derive(Deserialize)]
struct DelayParam {
    time: u64,
    message: String,
}

fn delay(params: DelayParam) -> Box<Future<Item = Response, Error = io::Error>> {
    let DelayParam { time, message } = params;
    let (tx, rx) = oneshot::channel();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(time));
        tx.send(()).unwrap();
    });
    rx.and_then(move |_| ok(Response::from_ok(message)))
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
        .boxed()
}


pub struct Handler;

impl Service for Handler {
    type Request = Request;
    type Response = Response;
    type Error = io::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        match req.method.as_str() {
            "0:function:the_answer" => ok(the_answer().into()).boxed(),
            "0:function:delay" => {
                match from_value(req.params) {
                    Ok(params) => delay(params),
                    Err(e) => ok(Response::from_err(e.to_string())).boxed(),
                }
            }
            m => {
                ok(Response::from_err(
                    format!("The method is not found: {:?}", m),
                )).boxed()
            }
        }
    }
}


pub struct Dummy;

impl NotifyService for Dummy {
    type Item = Notification;
    type Error = io::Error;
    type Future = FutureResult<(), Self::Error>;

    fn call(&self, _not: Notification) -> Self::Future {
        ok(())
    }
}
