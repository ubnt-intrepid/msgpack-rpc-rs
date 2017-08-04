extern crate msgpack_rpc;
extern crate futures;
extern crate tokio_core;
extern crate tokio_timer;
extern crate rmpv;
#[macro_use]
extern crate serde_derive;
extern crate serde;

use msgpack_rpc::{from_io, Handler, HandleResult};
use msgpack_rpc::io::{StdioStream, ChildProcessStream};
use std::time::Duration;
use futures::Future;
use futures::future::{empty, ok, join_all};
use tokio_core::reactor::Core;
use tokio_timer::{Timer, Sleep};
use rmpv::Value;
use rmpv::ext::{from_value, to_value};


fn delayed(interval: Duration) -> Sleep {
    Timer::default().sleep(interval)
}



#[derive(Serialize, Deserialize)]
struct DelayParam {
    interval: u64,
    message: String,
}

pub struct RootHandler;

impl Handler for RootHandler {
    fn handle_request(&self, method: &str, params: Value) -> HandleResult {
        match method {
            "0:function:the_answer" => ok(Ok(42u64.into())).boxed(),
            "0:function:delay" => {
                match from_value(params) {
                    Ok(DelayParam { interval, message }) => {
                        delayed(Duration::from_secs(interval))
                            .map_err(|_| ())
                            .map(move |_| Ok(message.into()))
                            .boxed()
                    }
                    Err(e) => ok(Err(e.to_string().into())).boxed(),
                }
            }
            m => ok(Err(format!("The method is not found: {:?}", m).into())).boxed(),
        }
    }
}

fn endpoint() {
    // create a pair of client/endpoint from an asynchronous I/O.
    let (_, endpoint, distributor) = from_io(StdioStream::new(4, 4));

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    // launch the RPC encpoint with given service handlers.
    distributor.launch(&handle);
    endpoint.launch(&handle, RootHandler);

    // start event loop infinitely.
    core.run(empty::<(), ()>()).unwrap();
}

fn main() {
    if let Some("--endpoint") = std::env::args().nth(1).as_ref().map(|s| s.as_str()) {
        endpoint();
        return;
    }

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let program = std::env::args().nth(0).unwrap();
    let child = ChildProcessStream::launch(program, vec!["--endpoint"], &handle).unwrap();

    let (client, _, distributor) = from_io(child);
    distributor.launch(&handle);

    let client = client.launch(&handle);
    core.run(join_all((0..10).map(move |i| {
        eprintln!("Request: {}", i);
        let response = if i == 4 {
            client.request(
                "0:function:delay",
                to_value(DelayParam {
                    interval: 1,
                    message: "Hi".into(),
                }).unwrap(),
            )
        } else {
            client.request("0:function:the_answer", Vec::<Value>::new())
        };
        response
            .and_then(|res| {
                eprintln!("Received: {:?}", res);
                ok(())
            })
            .map_err(|_| ())
    }))).unwrap();;
}
