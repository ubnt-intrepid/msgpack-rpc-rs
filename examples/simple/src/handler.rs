use msgpack_rpc::{Response, Handler};

use std::time::Duration;
use std::thread;
use futures::Future;
use futures::sync::oneshot;
use futures::future::{ok, BoxFuture};
use rmpv::Value;
use rmpv::ext::from_value;


fn the_answer() -> Result<u64, String> {
    Ok(42)
}


#[derive(Deserialize)]
struct DelayParam {
    time: u64,
    message: String,
}

fn delay(params: DelayParam) -> BoxFuture<Response, ()> {
    let DelayParam { time, message } = params;
    let (tx, rx) = oneshot::channel();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(time));
        tx.send(()).unwrap();
    });
    rx.and_then(move |_| ok(Response::from_ok(message)))
        .map_err(|_| ())
        .boxed()
}


pub struct RootHandler;

impl Handler for RootHandler {
    fn handle_request(&self, method: &str, params: Value) -> BoxFuture<Response, ()> {
        match method {
            "0:function:the_answer" => ok(the_answer().into()).boxed(),
            "0:function:delay" => {
                match from_value(params) {
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
