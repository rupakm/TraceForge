use traceforge::{send_msg, thread::current, Config};

const TEST_RUNS: i32 = 20;

#[test]
fn test_awaited_future() {
    for _ in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(traceforge::SchedulePolicy::Arbitrary)
                .build(),
            || {
                traceforge::future::block_on(async {
                    let main_tid = current().id();
                    send_msg(main_tid, format!("Hello from {}", current().id()));

                    let fut = traceforge::future::spawn(async move {
                        send_msg(main_tid, format!("Hello from {}", current().id()));
                    });

                    let greeting: String = traceforge::recv_msg_block();
                    println!("Got {:?}", greeting);

                    let _ = fut.await;
                });
            },
        );

        // This results in two execs as there are two messages that could be delivered.
        assert_eq!((2, 0), (stats.execs, stats.block));
    }
}

#[test]
fn test_unawaited_future() {
    for _ in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(traceforge::SchedulePolicy::Arbitrary)
                .with_verbose(1)
                .build(),
            || {
                traceforge::future::block_on(async {
                    let main_tid = current().id();
                    send_msg(main_tid, format!("Hello from {}", current().id()));

                    let _fut = traceforge::future::spawn(async move {
                        send_msg(main_tid, format!("Hello from {}", current().id()));
                    });

                    let greeting: String = traceforge::recv_msg_block();
                    println!("Got {:?}", greeting);
                });
            },
        );

        // This should result in two execs, since the future could run even if it's
        // not been awaited?
        // But this isn't what happens. Bug, or feature?
        assert_eq!((2, 0), (stats.execs, stats.block));
    }

    // If Rust future::spawn is meant to be like tokio, it doesn't match:
    // From https://tokio.rs/tokio/tutorial/spawning:

    // "Tasks are the unit of execution managed by the scheduler. Spawning the
    // task submits it to the Tokio scheduler, which then ensures that the
    // task executes when it has work to do."
}

#[test]
fn test_unawaited_future_can_run() {
    for _ in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(traceforge::SchedulePolicy::Arbitrary)
                .build(),
            || {
                traceforge::future::block_on(async {
                    let main_tid = current().id();
                    // NOTE: no message sent to the main thread here.

                    let _fut = traceforge::future::spawn(async move {
                        send_msg(main_tid, format!("Hello from {}", current().id()));
                    });

                    // The only message available is from the spawned future, but we didn't
                    // await it. If futures have to be awaited before, they can execute,
                    // this should result in a blocked execution.
                    let greeting: String = traceforge::recv_msg_block();
                    println!("Got {:?}", greeting);
                });
            },
        );

        assert_eq!((1, 0), (stats.execs, stats.block));
    }
}

#[test]
fn test_unjoined_thread() {
    for _ in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(traceforge::SchedulePolicy::Arbitrary)
                .build(),
            || {
                let main_tid = current().id();
                send_msg(main_tid, format!("Hello from {}", current().id()));

                let _t = traceforge::thread::spawn(move || {
                    send_msg(main_tid, format!("Hello from {}", current().id()));
                });

                let greeting: String = traceforge::recv_msg_block();
                println!("Got {:?}", greeting);
            },
        );

        // This should result in two execs, since the thread could run even if it's
        // not been joined?
        assert_eq!((2, 0), (stats.execs, stats.block));
    }
}

#[test]
fn test_abort_future() {
    let stats = traceforge::verify(Config::builder().build(), || {
        let main_tid = current().id();

        let _ = traceforge::future::spawn(async move {
            send_msg(main_tid, format!("Hello from {}", current().id()));
        });

        let greeting: String = traceforge::recv_msg_block();

        println!("Got {:?}", greeting);
        // Cancellation is in Drop
    });

    // Spawned futures are executed immediately and therefore the is
    // nothing to cancel for a send_msg.
    assert_eq!((1, 0), (stats.execs, stats.block));
}
