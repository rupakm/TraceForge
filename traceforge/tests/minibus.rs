use traceforge::thread;
use traceforge::*;
use SchedulePolicy::*;

/// A toy version of Microbus (single-offer, one standby).
/// Used for easier debugging

// Rust claims Offer is never constructed. Leaving commented
// until investigated further.
#[derive(Clone, PartialEq, Debug)]
enum Msg {
    // Offer,
    Ack,
    Replicate,
    StoreCommitted,
    MyStatus,
    Terminate,
    Config(
        (
            thread::ThreadId,
            thread::ThreadId,
            thread::ThreadId,
            thread::ThreadId,
        ),
    ),
}

fn recv_config_block() -> (
    thread::ThreadId,
    thread::ThreadId,
    thread::ThreadId,
    thread::ThreadId,
) {
    match recv_msg_block() {
        Msg::Config(config) => (config.0, config.1, config.2, config.3),
        _ => {
            assume!(false);
            panic!();
        }
    }
}

fn acceptor() {
    let (cl, _ac, cm, _st) = recv_config_block();

    send_msg(cm, Msg::Replicate);
    loop {
        match recv_msg_block() {
            // any mystatus will do for this version -- no failures
            Msg::MyStatus => send_msg(cl, Msg::Ack),
            Msg::Terminate => break,
            _ => panic!(),
        }
    }
}

fn committer() {
    let (_cl, ac, _cm, st) = recv_config_block();

    loop {
        match recv_msg_block() {
            Msg::Replicate => {
                send_msg(st, Msg::StoreCommitted);
                send_msg(ac, Msg::MyStatus);
            }
            Msg::Terminate => break,
            Msg::MyStatus => continue,
            _ => panic!(),
        }
    }
}

fn standby() {
    let (_cl, ac, cm, _st) = recv_config_block();

    loop {
        match recv_msg_block() {
            Msg::StoreCommitted => {
                send_msg(ac, Msg::MyStatus);
                send_msg(cm, Msg::MyStatus);
            }
            Msg::Terminate => break,
            _ => panic!(),
        }
    }
}

#[test]
#[serial_test::serial]
fn minibus_wait() {
    for cons in [ConsType::FIFO] {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(cons)
                .build(),
            move || {
                let acceptor = thread::spawn(acceptor);
                let committer = thread::spawn(committer);
                let standby = thread::spawn(standby);

                let config = (
                    thread::current().id(),
                    acceptor.thread().id(),
                    committer.thread().id(),
                    standby.thread().id(),
                );
                for tid in [&acceptor, &committer, &standby] {
                    send_msg(tid.thread().id(), Msg::Config(config));
                }

                match recv_msg_block() {
                    Msg::Ack => {
                        for tid in [acceptor, committer, standby] {
                            send_msg(tid.thread().id(), Msg::Terminate);
                        }
                    }
                    _ => panic!(),
                }
            },
        );
        assert_eq!(stats.execs, 9);
    }
}

#[test]
#[serial_test::serial]
fn minibus_nowait() {
    for cons in [ConsType::FIFO] {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(cons)
                .build(),
            move || {
                let acceptor = thread::spawn(acceptor);
                let committer = thread::spawn(committer);
                let standby = thread::spawn(standby);

                let config = (
                    thread::current().id(),
                    acceptor.thread().id(),
                    committer.thread().id(),
                    standby.thread().id(),
                );
                for tid in [&acceptor, &committer, &standby] {
                    send_msg(tid.thread().id(), Msg::Config(config));
                }

                for tid in [acceptor, committer, standby] {
                    send_msg(tid.thread().id(), Msg::Terminate);
                }
            },
        );
        assert_eq!(stats.execs, if cons == ConsType::MO { 15 } else { 13 });
    }
}
