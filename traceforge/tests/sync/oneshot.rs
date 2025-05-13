use traceforge::{sync::oneshot::*, *};
// this file shows some example usage of the Must's `sync::oneshot` library

#[test]
fn single_producer() {
    let f = || {
        let (tx, rx) = channel();
        let _t = thread::spawn(move || {
            let _ = tx.send(10);
        });
        traceforge::future::block_on(async {
            assert(rx.await.expect("ok") == 10);
        });
    };

    let stats = verify(
        Config::builder().with_keep_going_after_error(false).build(),
        f,
    );
    println!("Stats = {}, {}", stats.execs, stats.block);
}
