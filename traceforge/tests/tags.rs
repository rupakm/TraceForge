extern crate traceforge;

use traceforge::thread::{self, ThreadId};
use traceforge::*;
use SchedulePolicy::*;

#[derive(Clone, PartialEq, Debug)]
enum Msg {
    Val(i32),
    TID(ThreadId),
}

// This is a test that shows a non-trivial scenario with selective receives
#[test]
fn nontrivial_tag() {
    let stats = traceforge::verify(
        Config::builder()
            .with_keep_going_after_error(true)
            .with_policy(LTR)
            .with_cons_type(ConsType::FIFO)
            .with_verbose(1)
            .build(),
        || {
            let first = thread::spawn(move || {
                let _v1 = match traceforge::recv_tagged_msg_block(move |_, _| true) {
                    Msg::Val(v1) => v1,
                    _ => {
                        panic!()
                    }
                };
                let _v2 = match traceforge::recv_tagged_msg_block(move |_, _| true) {
                    Msg::Val(v2) => v2,
                    _ => {
                        panic!()
                    }
                };
            });
            let second = thread::spawn(move || {
                let first_id = match traceforge::recv_tagged_msg_block(move |_, t| {
                    t.is_some() && t.unwrap() == 1
                }) {
                    Msg::TID(first_id) => first_id,
                    _ => {
                        panic!()
                    }
                };
                traceforge::send_tagged_msg(first_id, 1, Msg::Val(1));
                traceforge::send_tagged_msg(first_id, 2, Msg::Val(2));
            });
            traceforge::send_tagged_msg(second.thread().id(), 1, Msg::TID(first.thread().id()));
            traceforge::send_tagged_msg(first.thread().id(), 2, Msg::Val(3));
            traceforge::send_tagged_msg(first.thread().id(), 2, Msg::Val(4));
        },
    );
    println!("Number of executions explored {}", stats.execs);
    println!("Number of blocked executions explored {}", stats.block);
    assert_eq!(stats.execs, 4); // all prefixes of length 2 of interleavings between [1;2] and [3,4]
}

// This is a test that shows a non-trivial scenario with selective receives
#[test]
fn tag_on_sender() {
    let stats = traceforge::verify(
        Config::builder()
            .with_keep_going_after_error(true)
            .with_policy(LTR)
            .with_cons_type(ConsType::FIFO)
            .with_verbose(1)
            .build(),
        || {
            let main_thread = thread::current().id();
            let first = thread::spawn(move || {
                let _v = match traceforge::recv_tagged_msg_block(move |tid, tag| {
                    tid == main_thread && tag.is_some() && tag.unwrap() == 2
                }) {
                    Msg::Val(v2) => v2,
                    _ => {
                        panic!()
                    }
                };
            });
            traceforge::send_tagged_msg(first.thread().id(), 2, Msg::Val(4));
        },
    );
    println!("Number of executions explored {}", stats.execs);
    println!("Number of blocked executions explored {}", stats.block);
    assert_eq!(stats.execs, 1);
}
