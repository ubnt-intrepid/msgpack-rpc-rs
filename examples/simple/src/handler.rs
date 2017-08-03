use msgpack_rpc::{Handler, HandleResult};

use std::time::Duration;
use std::thread;
use futures::Future;
use futures::sync::oneshot;
use futures::future::ok;
use rmpv::Value;
use rmpv::ext::from_value;


#[derive(Deserialize)]
struct DelayParam {
    time: u64,
    message: String,
}

fn delay(params: DelayParam) -> HandleResult {
    let DelayParam { time, message } = params;
    let (tx, rx) = oneshot::channel();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(time));
        tx.send(()).unwrap();
    });
    rx.and_then(move |_| ok(Ok(message.into())))
        .map_err(|_| ())
        .boxed()
}


pub struct RootHandler;

impl Handler for RootHandler {
    fn handle_request(&self, method: &str, params: Value) -> HandleResult {
        match method {
            "0:function:the_answer" => ok(Ok(42u64.into())).boxed(),
            "0:function:delay" => {
                match from_value(params) {
                    Ok(params) => delay(params),
                    Err(e) => ok(Err(e.to_string().into())).boxed(),
                }
            }
            m => ok(Err(format!("The method is not found: {:?}", m).into())).boxed(),
        }
    }
}
