use traceforge::{
    nondet, recv_msg_block, recv_tagged_msg_block, send_msg, send_tagged_msg,
    thread::{self, current_id, spawn, ThreadId},
    verify, Config,
};

mod utils;

#[derive(Clone, Debug, PartialEq)]
enum Message {
    Init(Vec<ThreadId>),
    TimerTick,
    Ping(ThreadId),
    Pong,
}

/// Each member receives 1 Init message.
/// Then it sends a Timer message to itself.
/// When it receives a Timer message, it send Ping to all the peers, and then it sends itself another Timer message
/// If it receives a Ping, it responds Pong.
/// When it receives Pong it exits.
///
/// It seems like this should always terminate after a few steps, but under P2P semantics, the TimerTick
/// can be delivered to self before the Pong is delivered, forever. You could think of this as a
/// livelock which would be caused by placing a higher priority on delivering TimerTick messages
/// in a deterministically scheduled system.
///
fn member_loop() {
    let init: Message = recv_tagged_msg_block(|_, tag| tag == Some(0));
    send_msg(current_id(), Message::TimerTick);
    if let Message::Init(members) = init {
        loop {
            match recv_msg_block() {
                Message::Init(_) => {
                    traceforge::assume!(false);
                }
                Message::TimerTick => {
                    for &peer in &members {
                        if peer != thread::current_id() {
                            send_msg(peer, Message::Ping(thread::current_id()));
                        }
                    }
                    send_msg(thread::current_id(), Message::TimerTick);
                }
                Message::Ping(pinger) => {
                    send_msg(pinger, Message::Pong);
                }
                Message::Pong => {
                    return;
                }
            }
        }
    }
}

fn bug_triggering_config() -> Config {
    // with_trace_out, with_dot_out, etc., are not that well tested in other tests.
    Config::builder()
        .with_trace_out("/tmp/unbounded_execution.traces")
        .with_dot_out("/tmp/unbounded_execution.dot")
        .with_error_trace("/tmp/unbounded_execution.fail")
        .build()
}

#[test]
fn test_simple_unbounded_loop() {
    verify(bug_triggering_config(), || loop {
        let _: Option<i32> = traceforge::recv_msg();
    });
}

/// This exposes a strange Must corner case / performance issue.
/// Must can almost instantly model check the previous unbounded loop
/// But with a single call to nondet() like this, it seems that Must will
/// never finish, or else it will take a prohibitive amount of time.
#[ignore = "the test takes too long to run"]
#[test]
fn test_simple_unbounded_loop_with_useless_nondet() {
    verify(bug_triggering_config(), || {
        let _useless_nondet: bool = nondet();
        loop {
            let _: Option<i32> = traceforge::recv_msg();
        }
    });
}

fn scenario() {
    let member1 = spawn(member_loop);
    let member2 = spawn(member_loop);

    let tids = vec![member1.thread().id(), member2.thread().id()];

    send_tagged_msg(tids[0], 0, Message::Init(tids.clone()));
    send_tagged_msg(tids[1], 0, Message::Init(tids));

    let _ = member1.join();
    let _ = member2.join();
}

#[test]
fn test_unbounded_protocol() {
    verify(bug_triggering_config(), || {
        scenario();
    });
}
