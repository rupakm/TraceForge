## TraceForge: Systematic Concurrency Exploration for Distributed Systems

TraceForge is a library to perform systematic exploration of the space of concurrent
message interleavings in distributed systems and their specifications directly in Rust.
A TraceForge test models a distributed system by spawning a set of threads (representing
processes in a distributed system) that communicate via message passing using TraceForge's API. 
TraceForge implements a specialized runtime to control the scheduling of messages and 
to perform systematic exploration of possible message interleavings.
By default, TraceForge looks for assertion violations in the code, but one can also write more complex
specifications given as state machines or those that check a property on termination.


## Getting started

Consider the following message passing implementation:

```
use traceforge::thread;

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Work,
    Terminate,
}

fn example() {
    let t1 = thread::spawn(move || {
        let mut ready = true;
        let mut i = 0;
        loop {
            let m = traceforge::recv_msg_block();
            match m {
                Msg::Work => assert!(ready),
                Msg::Terminate => ready = false,
            }
            i = i + 1;
            if i == 2 {
                break;
            }
        }
    });
    let t1_id = t1.thread().id();

    let t2 = thread::spawn(move || {
        traceforge::send_msg(t1_id, Msg::Work);
    });
    traceforge::send_msg(t1_id.clone(), Msg::Terminate);

    let _ = t1.join();
    let _ = t2.join();
}

```

Here, `thread::spawn` is a TraceForge API call to spawn a process, `send_msg` is a call to send a message, and `recv_msg_block`
is a blocking message receive.
The assertion can fail because the two messages to `t1` can race, and `Terminate` can be received before `Work`.
However, finding and reproducing these kinds of bugs using standard testing techniques are difficult: they depend on a precise scheduling
of message deliveries that may be missed in random testing.

In contrast, TraceForge systematically explores all relevant message orderings to find such bugs; moreover, the specific schedules that lead to a bug
are reproducible.

TraceForge tests the function by wrapping the execution to the `verify` function:

```
fn test_example() {
    let stats = traceforge::verify(
        traceforge::Config::builder().build(),
        example,
    );
    println!("Stats = {}, {}", stats.execs, stats.block);
}
```

The test will detect the assertion violation.

## Building TraceForge

To build TraceForge, run

```
cargo build
```

To run the unit tests, run
```
cargo test
```
We have put the above simple example in `traceforge/examples`. You can run it by executing
```
cd traceforge
cargo run --example simple -- --random 
cargo run --example simple -- --forge
```
The `--random` option runs a random test. The `--forge` option runs systematic tests (and should always panic).
While testing is very likely to find the assertion violation in this simple test, you can try running the `hard_to_find` example
in `examples` by running:
```
cd traceforge
cargo run --example hard_to_find -- --random 
cargo run --example hard_to_find -- --forge
```
Random testing rarely finds the violation, but traceforge always does.

## Scaling Systematic Exploration

In order to scale, TraceForge implements an [optimal dynamic partial order reduction algorithm](https://dl.acm.org/doi/10.1145/3689778).
The algorithm defines an equivalence relation among executions and  only explores one representative
from each equivalence class.
The equivalence relation considers two executions as equivalent if they agree on the messages received by each process
(technically, a reads-from equivalence).

When a specification is violated, TraceForge provides a trace listing the order of messages sent
which led to the violation. The trace can be replayed under a debugger to investigate it further.
Importantly, if TraceForge finishes without finding a counterexample, we know that no counterexample exists, 
under the assumptions of the test scenario.

## Related Tools

TraceForge is inspired by other tools for checking concurrent Rust code, such as [Loom](http://github.com/tokio-rs/loom) and [Shuttle](https://github.com/awslabs/shuttle).


## Contributing

See [CONTRIBUTING](CONTRIBUTING.md#security-issue-notifications) for more information.

## License

This project is licensed under the Apache-2.0 License.

