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
use tokio_timer::Timer;
use rmpv::Value;
use rmpv::ext::{from_value, to_value};


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
                        Timer::default()
                            .sleep(Duration::from_secs(interval))
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
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    // Create an asynchronous I/O of standard input/standard output.
    let stdio = StdioStream::new(4);

    // Launch a RPC endpoint with given service handlers.
    let (_, endpoint, distributor) = from_io(stdio);
    distributor.launch(&handle);
    endpoint.launch(&handle, RootHandler);

    // start event loop infinitely.
    core.run(empty::<(), ()>()).unwrap();
}

fn client() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    // Spawn the process as a RPC endpoint.
    let program = std::env::args().nth(0).unwrap();
    let child = ChildProcessStream::launch(&handle, program, vec!["--endpoint"]).unwrap();

    // Create a RPC client associated with the child process spawned above.
    let client = {
        let (client, _, distributor) = from_io(child);
        distributor.launch(&handle);
        client.launch(&handle)
    };

    let task = join_all((0..10).map(move |i| {
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

        response.and_then(|res| {
            eprintln!("Response: {:?}", res);
            ok(())
        })
    }));

    core.run(task).unwrap();;
}

fn main() {
    if let Some("--endpoint") = std::env::args().nth(1).as_ref().map(|s| s.as_str()) {
        endpoint();
    } else {
        client();
    }
}
