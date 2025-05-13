use traceforge::thread::{self, spawn, spawn_daemon};
use traceforge::{nondet, recv_msg_block, send_tagged_msg, spawn_symmetric, Stats};
use traceforge::{send_msg, thread::current_id, verify, Config};

use std::cell::RefCell;
use std::panic::catch_unwind;
use std::rc::Rc;

use utils::assert_panic_contains;
mod utils;

/// This illustrates something that is NOT supposed to be done with TraceForge--
/// A function that is supposed to be deterministic should have the same
/// value for every execution. This counter starts at 0, but because it is
/// not reset between TraceForge executions, it will have a larger value in the next
/// execution.
fn get_value_that_is_supposed_to_be_deterministic() -> u32 {
    COUNTER.with(|counter| {
        let n = *counter.borrow();
        *counter.borrow_mut() += 1;
        n
    })
}

thread_local! {
    static COUNTER: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
}

/// This function has some pointless nondeterminism in order to force TraceForge to
/// perform a revisit.
fn do_something_that_requires_backtracking() {
    let t1 = spawn(|| {
        let _: String = recv_msg_block();
    });
    send_msg(t1.thread().id().clone(), "1".to_string());
    let _ = spawn(move || {
        send_msg(t1.thread().id(), "2".to_string());
    });
}

/// This test shows that when a TraceForge model program is nondeterministic,
/// it can result in the the delivery of a message that is retained from a
/// previous execution rather than the delivery of a message actually sent.
///
/// The first execution of the model program will work perfectly, but during
/// a revisit, it will not receive the message that was sent; instead, it will
/// receive the message that was sent by the previous execution.
///
/// The behavior seen is an impossible behavior according to TraceForge send/receive
/// semantics. This results in customer chaos and great difficulty in
/// understanding complex model programs because we can't easily tell if the
/// strange behavior is (1) actually allowed by the model, or (2) caused by
/// nondeterminism in the model or (3) a TraceForge bug.
///
/// Ideally TraceForge should be able to detect this incorrect program, but it cannot
/// because when the execution graph is revisited, we clear out all of the
/// values cached from the previous execution.
#[test]
#[should_panic(expected = "Incorrect TraceForge Program")]
#[ignore] // Can't pass unless we save the values from the previous execution.
fn test_nondet_guardrail_send() {
    let stats = verify(Config::builder().build(), move || {
        let n = get_value_that_is_supposed_to_be_deterministic();

        let sent_msg = if n == 0 { "foo" } else { "bar" };
        send_msg(current_id(), sent_msg);
        do_something_that_requires_backtracking();
        let received_msg: &str = recv_msg_block();

        // Since we just sent it, it HAS TO BE have the same value as
        // expected_msg. RIGHT?
        println!("Sent {sent_msg} and received {received_msg}");

        // This anomaly is detected by the model code, but instead,
        // TraceForge itself should be able to detect it so that the customer
        // does not need to program so defensively.
    });
    assert_eq!((stats.execs, stats.block), (2, 0));
}

#[test]
#[should_panic(expected = "Incorrect TraceForge Program")]
fn test_nondet_guardrail_send_tag() {
    let stats = verify_with_backtrackers(|| {
        let tag = get_value_that_is_supposed_to_be_deterministic();
        send_tagged_msg(current_id(), tag, "foo");
    });
    assert_eq!((stats.execs, stats.block), (2, 0));
}

#[test]
#[should_panic(expected = "Incorrect TraceForge Program")]
#[ignore] // Can't pass unless we save the values from the previous execution.
fn test_nondet_guardrail_joined_value() {
    let stats = verify_with_backtrackers(|| {
        let jh = spawn(|| get_value_that_is_supposed_to_be_deterministic());
        jh.join().unwrap();
    });
    assert_eq!((stats.execs, stats.block), (2, 0));
}

#[test]
#[should_panic(expected = "Incorrect TraceForge Program")]
fn test_nondet_guardrail_joined_wrong_thread() {
    let stats = verify_with_backtrackers(|| {
        let jh0 = spawn(|| ());
        let jh1 = spawn(|| ());

        let n = get_value_that_is_supposed_to_be_deterministic();
        if n == 0 {
            jh0.join().unwrap();
        } else {
            jh1.join().unwrap();
        }
    });
    assert_eq!((stats.execs, stats.block), (2, 0));
}

#[test]
#[should_panic(expected = "Incorrect TraceForge Program")]
fn test_nondet_guardrail_thread_name_changed() {
    let stats = verify_with_backtrackers(|| {
        let n = get_value_that_is_supposed_to_be_deterministic();
        let _ = thread::Builder::new().name(format!("name{n}")).spawn(|| ());
    });
    assert_eq!((stats.execs, stats.block), (2, 0));
}

#[test]
#[should_panic(expected = "Incorrect TraceForge Program")]
fn test_nondet_guardrail_thread_daemon_changed() {
    let stats = verify_with_backtrackers(|| {
        let n = get_value_that_is_supposed_to_be_deterministic();
        let _ = if n == 0 {
            spawn(|| ());
        } else {
            spawn_daemon(|| ());
        };
    });
    assert_eq!((stats.execs, stats.block), (2, 0));
}

#[test]
#[should_panic(expected = "Incorrect TraceForge Program")]
fn test_nondet_guardrail_thread_sym_cid_changed() {
    let stats = verify_with_backtrackers(|| {
        let n = get_value_that_is_supposed_to_be_deterministic();
        let t1 = spawn_daemon(|| ()).thread().id();
        let t2 = spawn_daemon(|| ()).thread().id();
        let _ = spawn_symmetric(|| (), if n == 0 { t1 } else { t2 });
    });
    assert_eq!((stats.execs, stats.block), (2, 0));
}

fn verify_with_backtrackers<F>(f: F) -> Stats
where
    F: Fn() + Send + Sync + 'static,
{
    verify(Config::builder().build(), move || {
        f();
        let t1 = spawn(|| {
            println!("Got {}", recv_msg_block::<String>());
        });
        send_msg(t1.thread().id(), "1".to_string());
        let _ = spawn(move || {
            send_msg(t1.thread().id(), "2".to_string());
        });
    })
}

#[test]
fn detect_many_bad_programs() {
    for i in 0..=4 {
        let res = catch_unwind(|| {
            verify_with_backtrackers(move || {
                let n = get_value_that_is_supposed_to_be_deterministic() % 2;
                let jh = spawn(|| ());
                send_tagged_msg(current_id(), 5, "self".to_string());

                // Pick something to do based on i.
                match i + n {
                    0 => send_msg(jh.thread().id(), n),
                    1 => {
                        let _: String = recv_msg_block();
                    }
                    2 => {
                        let _ = jh.join();
                    }
                    3 => {
                        let _ = nondet();
                    }
                    4 => {
                        use traceforge::Nondet;
                        let _ = (n as usize..3).nondet();
                    }
                    _ => {
                        // Nothing
                    }
                };
            });
        });
        println!("After Run, i={}", i);
        assert_panic_contains(res, "Incorrect TraceForge Program");
    }
}
