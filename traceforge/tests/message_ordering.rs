use traceforge::*;
use serial_test::serial;
use utils::assert_panic_msg;

mod utils;

fn client_tagged(n: u32) {
    let mut messages = Vec::new();
    for i in 1..n + 1 {
        let v: u32 = traceforge::recv_tagged_msg_block(move |_, x| x.is_some() && x.unwrap() == i);
        println!("Received message is {}", v);
        messages.push(v);
    }
    let res = vec![4, 3, 2, 1, 0];
    traceforge::assert(messages != res);
}

fn client_untagged(n: u32) {
    let mut messages = Vec::new();
    let v: u32 = traceforge::recv_tagged_msg_block(move |_, x| x.is_some() && x.unwrap() == n);
    messages.push(v);
    for _i in 0..n - 1 {
        let v: u32 = traceforge::recv_msg_block();
        println!("Received message is {}", v);
        messages.push(v);
    }
    traceforge::assert(messages[0] == 0);
}

fn mainmodel_tagged() {
    let n = 5;
    let client = traceforge::thread::spawn(move || client_tagged(n));
    let client_id = client.thread().id();

    for i in 0..n {
        println!("Sending message {} with tag {}", i, n - i);
        traceforge::send_tagged_msg(client_id, n - i, i);
    }
}

fn mainmodel_untagged() {
    let n = 5;
    let client = traceforge::thread::spawn(move || client_untagged(n));
    let client_id = client.thread().id();

    for i in 0..n {
        println!("Sending message {} with tag {}", i, n - i);
        traceforge::send_tagged_msg(client_id, n - i, i);
    }
}

fn generate_counterexample(filename: &str) {
    let res = std::panic::catch_unwind(|| {
        let _stats = traceforge::verify(
            Config::builder().with_error_trace(filename).build(),
            mainmodel_tagged,
        );
    });
    assert_panic_msg(res, "assertion failed: cond");
}

#[test]
#[serial]
fn ordering_test1() {
    generate_counterexample("/tmp/ordering_test1.json");
}

#[test]
#[serial]
fn replay_test1() {
    let filename = "/tmp/replay_test1.json";

    let _ = std::fs::remove_file(filename);
    generate_counterexample(filename);

    // Make sure that we get exactly the same panic msg when replaying.
    let res = std::panic::catch_unwind(|| {
        traceforge::replay(mainmodel_tagged, filename);
    });
    assert_panic_msg(res, "assertion failed: cond");
}

#[test]
#[serial]
fn ordering_test2() {
    let stats = traceforge::verify(
        Config::builder().with_cons_type(ConsType::MO).build(),
        mainmodel_untagged,
    );

    assert_eq!(stats.execs, 1); // just one exec with FIFO
}
