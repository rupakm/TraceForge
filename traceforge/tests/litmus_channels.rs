use traceforge::*;
use channel::Builder;
use loc::CommunicationModel;

const TEST_RUNS: u32 = 20;

fn ltr_config() -> Config {
    Config::builder()
        .with_policy(SchedulePolicy::Arbitrary)
        .build()
}

fn arbitrary_config() -> Config {
    Config::builder()
        .with_policy(SchedulePolicy::Arbitrary)
        .build()
}

// smoke test, checking channel inequality
#[test]
fn smoke() {
    let stats = traceforge::verify(ltr_config(), || {
        let chan = Builder::new().with_name("foo").build();
        let chanp = Builder::new().with_name("bar").build();
        chan.0.send_msg(42);
        // message to another channel should be irrelevant
        chanp.0.send_msg(17);
        let _ = chan.1.recv_msg_block();
    });
    assert_eq!(stats.execs, 1);
}

// channels are the "same" if their ids are the same
#[test]
fn equal_channels() {
    let stats = traceforge::verify(ltr_config(), || {
        let id = "foo";
        let chan1 = Builder::new().with_name(id).build();
        let chan2 = Builder::<i32>::new().with_name(id).build();
        chan1.0.send_msg(1);
        let _ = chan2.1.recv_msg();
    });
    assert_eq!(stats.execs, 2);
}

// plain forward revisit works
#[test]
fn forward_revisit() {
    for comm in [CommunicationModel::NoOrder, CommunicationModel::LocalOrder] {
        let stats = traceforge::verify(ltr_config(), move || {
            let chan = Builder::new().with_comm(comm).build();
            chan.0.send_msg(1);
            chan.0.send_msg(2);
            let _ = thread::spawn(move || {
                let _ = chan.1.recv_msg();
            });
        });
        match comm {
            CommunicationModel::NoOrder => assert_eq!(stats.execs, 3),
            CommunicationModel::LocalOrder => assert_eq!(stats.execs, 2),
            _ => {}
        }
    }
}

// plain backward revisit works
#[test]
fn backward_revisit() {
    for comm in [CommunicationModel::NoOrder, CommunicationModel::LocalOrder] {
        let stats = traceforge::verify(ltr_config(), move || {
            let (sender, receiver) = Builder::new().with_comm(comm).build();
            let _ = thread::spawn(move || {
                sender.send_msg(1);
            });
            let _ = receiver.recv_msg();
        });
        assert_eq!(stats.execs, 2);
    }
}

// do not revisit your porf-prefix (create->begin *is* porf)
#[test]
fn no_prefix_rev_cb() {
    let stats = traceforge::verify(ltr_config(), || {
        let (sender, receiver) = Builder::new().build();
        let _ = receiver.recv_msg();
        let _ = thread::spawn(move || {
            sender.send_msg(1);
        });
    });
    assert_eq!(stats.execs, 1);
}

// do not revisit your porf-prefix (end->join *is* porf)
#[test]
fn no_prefix_rev_ej() {
    let statsp = traceforge::verify(ltr_config(), || {
        let (sender, receiver) = Builder::new().build();
        let p = thread::spawn(move || {
            let _ = receiver.recv_msg();
        });
        p.join().unwrap();
        sender.send_msg(1);
    });
    assert_eq!(statsp.execs, 1);
}

// revisits and optimality
#[test]
fn maximality1() {
    let stats = traceforge::verify(ltr_config(), || {
        let (sender, receiver) = Builder::new().build();
        let _ = thread::spawn(move || {
            sender.send_msg(1);
        });
        let _ = receiver.recv_msg();
        let _ = receiver.recv_msg();
    });
    assert_eq!(stats.execs, 3);
}

#[test]
fn maximality2() {
    for comm in [CommunicationModel::NoOrder, CommunicationModel::LocalOrder] {
        for _ in 0..TEST_RUNS {
            let stats = traceforge::verify(arbitrary_config(), move || {
                let (sender, receiver) = Builder::new().with_comm(comm).build();
                let _ = thread::spawn(move || {
                    sender.send_msg(1);
                    sender.send_msg(2);
                });
                let _ = receiver.recv_msg();
                let _ = receiver.recv_msg();
            });
            match comm {
                CommunicationModel::NoOrder => assert_eq!(stats.execs, 7),
                CommunicationModel::LocalOrder => assert_eq!(stats.execs, 4),
                _ => {}
            }
        }
    }
}

#[test]
fn stress_max() {
    for comm in [CommunicationModel::NoOrder, CommunicationModel::LocalOrder] {
        for r in 0..10 {
            for _ in 0..TEST_RUNS {
                let stats = traceforge::verify(arbitrary_config(), move || {
                    let (sender, receiver) = Builder::new().with_comm(comm).build();
                    let _ = thread::spawn(move || {
                        let s = if comm == CommunicationModel::NoOrder {
                            2
                        } else {
                            r
                        };
                        for _ in 0..s {
                            sender.send_msg(1);
                        }
                    });
                    for _ in 0..r {
                        let _ = receiver.recv_msg();
                    }
                });
                let expected = if comm == CommunicationModel::NoOrder {
                    (r * r + r + 1) as usize
                } else {
                    2_i32.pow(r) as usize
                };
                assert_eq!(stats.execs, expected);
            }
        }
    }
}

// smoke test for blocking, checking channel inequality
#[test]
fn block() {
    let stats = traceforge::verify(ltr_config(), || {
        let chan = Builder::new().build();
        let () = chan.1.recv_msg_block();
    });
    assert_eq!(stats.execs, 0);
    assert_eq!(stats.block, 1);
}

// a blocking read cannot read from a send that has been read
#[test]
fn rrb_s() {
    let stats = traceforge::verify(ltr_config(), move || {
        let (sender, receiver) = Builder::new().build();
        let _ = thread::spawn(move || {
            sender.send_msg(1);
        });
        let _ = receiver.recv_msg();
        let _ = receiver.recv_msg_block();
    });
    assert_eq!(stats.execs, 1);
    assert_eq!(stats.block, 1);
}

#[test]
fn blocking_rev() {
    let stats = traceforge::verify(ltr_config(), move || {
        let (sender, receiver) = Builder::new()
            .with_comm(CommunicationModel::NoOrder)
            .build();
        let _ = thread::spawn(move || {
            sender.send_msg(1);
            sender.send_msg(2);
            sender.send_msg(3);
        });
        let _ = receiver.recv_msg_block();
        let _ = receiver.recv_msg_block();
    });
    assert_eq!(stats.execs, 6);
}

// simple select smoke test
#[test]
fn select() {
    let stats = traceforge::verify(ltr_config(), move || {
        let ch1 = Builder::new()
            .with_comm(CommunicationModel::NoOrder)
            .build();
        let ch2 = Builder::new()
            .with_comm(CommunicationModel::NoOrder)
            .build();
        ch1.0.send_msg(1);
        ch2.0.send_msg(1);
        let _ = traceforge::select_msg([&ch1.1, &ch2.1].iter(), CommunicationModel::NoOrder)
            .map(|x| x.0);
    });
    assert_eq!(stats.execs, 3);
}

#[test]
fn select_rev() {
    for comm in [CommunicationModel::NoOrder, CommunicationModel::LocalOrder] {
        for _ in 0..TEST_RUNS {
            let stats = traceforge::verify(arbitrary_config(), move || {
                let (sender1, receiver1) = Builder::new().with_comm(comm).build();
                let (sender2, receiver2) = Builder::new().with_comm(comm).build();
                let (sender3, receiver3) = Builder::new().with_comm(comm).build();
                let _ = thread::spawn(move || {
                    sender1.send_msg(1);
                    sender2.send_msg(2);
                    sender3.send_msg(3);
                });
                let _ = traceforge::select_msg_block([&receiver1, &receiver2].iter(), comm).0;
                let _ = traceforge::select_msg_block([&receiver1, &receiver3].iter(), comm).0;
                let _ = traceforge::select_msg_block([&receiver2, &receiver3].iter(), comm).0;
            });
            if comm == CommunicationModel::NoOrder {
                // two non-overlapping ways (1,3,2 and 2,1,3)
                assert_eq!(stats.execs, 2);
                // one way to block: both 2 and 3 are read by the time the last select is executed
                assert_eq!(stats.block, 1);
            } else {
                // but 2,1,3 is inconsistent: the first receive cannot read 2
                assert_eq!(stats.execs, 1);
                // but it's inconsistent
                assert_eq!(stats.block, 0);
            }
        }
    }
}

// check that the results are stable
#[test]
fn three_arbitrary_threads() {
    for _ in 0..TEST_RUNS {
        let stats = traceforge::verify(arbitrary_config(), move || {
            let (sender1, receiver1) = Builder::new().build();
            let (sender2, receiver2) = Builder::new().build();
            let (sender3, receiver3) = Builder::new().build();
            let _ = thread::spawn(move || {
                sender1.send_msg(1);
                let _ = receiver2.recv_msg();
            });
            let _ = thread::spawn(move || {
                sender2.send_msg(2);
                sender3.send_msg(3);
            });
            let comm = CommunicationModel::default();
            let _ = traceforge::select_msg_block([&receiver1, &receiver3].iter(), comm).0;
            let _ = traceforge::select_msg([&receiver1, &receiver3].iter(), comm).map(|x| x.0);
        });
        assert_eq!(stats.execs, 8);
        assert_eq!(stats.block, 0);
    }
}

#[test]
fn legacy_channel() {
    for _ in 0..TEST_RUNS {
        let stats = traceforge::verify(arbitrary_config(), || {
            let h = thread::spawn(move || {
                let _: Option<i32> = traceforge::recv_msg();
            });

            if (<bool>::nondet)() {
                traceforge::send_msg(h.thread().id(), 42);
            } else {
                traceforge::send_msg(h.thread().id(), 17);
            }
        });
        assert_eq!(stats.execs, 4);
        assert_eq!(stats.block, 0);
    }
}

#[test]
fn select_index() {
    let _stats = traceforge::verify(ltr_config(), move || {
        let (sender1, receiver1) = Builder::new()
            .with_comm(CommunicationModel::NoOrder)
            .build();
        let (sender2, receiver2) = Builder::new()
            .with_comm(CommunicationModel::NoOrder)
            .build();
        let chans = [&receiver1, &receiver2];
        sender1.send_msg(42u32);
        sender2.send_msg(43u32);
        let comm = CommunicationModel::default();
        let (v, i) = traceforge::select_msg_block(chans.iter(), comm);
        assert_eq!(v as usize, i + 42);
    });
}

#[test]
#[should_panic(expected = "Detected duplicate channel")]
fn select_no_duplicate_match() {
    let stats = traceforge::verify(ltr_config(), move || {
        let (sender1, receiver1) = Builder::new().with_name(42).build();
        let (_sender2, receiver2) = Builder::new().with_name(42).build();
        let chans = [&receiver1, &receiver2];
        sender1.send_msg(());
        let comm = CommunicationModel::default();
        let () = traceforge::select_msg_block(chans.iter(), comm).0;
    });
    // If we do not panic, we need to make sure that there's only one execution (?)
    assert_eq!(stats.execs, 1);
}
