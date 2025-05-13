use traceforge::thread::ThreadId;
use traceforge::thread::{self, main_thread_id};
use traceforge::*;
use SchedulePolicy::*;

mod utils;

/// Test names are of the form X_X_..._Y where each of the X
/// identifiers is a string composed of the following characters:
///
///   - r: receive
///   - s: send
///   - j: join
///
/// while underscores denote thread separation. The last identifier Y
/// does not represent a separate thread, but rather is a string
/// containing extra information about the testcase. For example:
///
///   - `rr_ss` is a program where the first thread does two receives and
/// the second one two sends.
///   - `r_s_assm` is a program where the first thread does a receive
///   the second one a send, and there is an assume on some value.
///
/// We do not distinguish between blocking and non-blocking receives
/// yet.

const TEST_RUNS: u32 = 20;

#[test]
fn r_nondet() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(Config::builder().with_policy(Arbitrary).build(), || {
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
    }
}

#[test]
fn r_choice_rangeinclusive() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_verbose(0)
                .build(),
            || {
                let h = thread::spawn(move || {
                    let _: Option<i32> = traceforge::recv_msg();
                });
                let r = 0_usize..=4_usize;
                let v = r.nondet();
                if v == 0 {
                    traceforge::send_msg(h.thread().id(), 42);
                } else if v == 1 {
                    traceforge::send_msg(h.thread().id(), 17);
                } else {
                    traceforge::send_msg(h.thread().id(), 33);
                }
            },
        );
        assert_eq!(stats.execs, 10);
    }
}
#[test]
fn r_choice_range() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_verbose(0)
                .build(),
            || {
                let h = thread::spawn(move || {
                    let _: Option<i32> = traceforge::recv_msg();
                });
                let r = 0_usize..4_usize;
                let v = r.nondet();

                if v == 0 {
                    traceforge::send_msg(h.thread().id(), 42);
                } else if v == 1 {
                    traceforge::send_msg(h.thread().id(), 17);
                } else {
                    traceforge::send_msg(h.thread().id(), 33);
                }
            },
        );
        assert_eq!(stats.execs, 8);
    }
}

#[test]
fn test_named_thread() {
    let stats = traceforge::verify(
        Config::builder()
            .with_verbose(1)
            .with_policy(Arbitrary)
            .build(),
        || {
            let h1 = thread::Builder::new()
                .name("H1".to_owned())
                .spawn(move || {
                    let _m1: Option<u32> = traceforge::recv_msg();
                })
                .unwrap();
            let h2 = thread::Builder::new()
                .name("H2".to_owned())
                .spawn(move || {
                    let _m2: Option<thread::ThreadId> = traceforge::recv_msg();
                })
                .unwrap();
            let h3 = thread::Builder::new()
                .name("H3".to_owned())
                .spawn(move || match traceforge::recv_msg_block() {
                    Some(t) => {
                        traceforge::send_msg(t, 42 as u32);
                    }
                    _ => panic!(),
                })
                .unwrap();
            traceforge::send_msg(h3.thread().id(), Some(h1.thread().id()));
            traceforge::send_msg(h2.thread().id(), h1.thread().id());
        },
    );
    assert_eq!(stats.execs, 4);
}

#[test]
fn s_s_rr() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::MO)
                .build(),
            || {
                let sh1 = thread::spawn(move || {
                    let r = traceforge::recv_msg();
                    if r.is_some() {
                        traceforge::send_msg(r.unwrap(), 1);
                    }
                });
                let sh2 = thread::spawn(move || {
                    let r = traceforge::recv_msg();
                    if r.is_some() {
                        traceforge::send_msg(r.unwrap(), 2);
                    }
                });
                let rh = thread::spawn(move || {
                    let _: Option<i32> = traceforge::recv_msg();
                    let _: Option<i32> = traceforge::recv_msg();
                });

                traceforge::send_msg(sh1.thread().id(), rh.thread().id());
                traceforge::send_msg(sh2.thread().id(), rh.thread().id());
            },
        );
        assert_eq!(stats.execs, 2 * 4 + 2 * 3); // + 1); // 15 execs under MO
                                                // 14 execs under FIFO
    }
}

#[test]
#[should_panic(expected = "assertion failed")]
fn s_s_rr_wrong() {
    traceforge::verify(Config::builder().with_policy(Arbitrary).build(), || {
        let sh1 = thread::spawn(move || {
            let r = traceforge::recv_msg();
            if r.is_some() {
                traceforge::send_msg(r.unwrap(), 1);
            }
        });
        let sh2 = thread::spawn(move || {
            let r = traceforge::recv_msg();
            if r.is_some() {
                traceforge::send_msg(r.unwrap(), 2);
            }
        });
        let rh = thread::spawn(move || {
            let m1: Option<i32> = traceforge::recv_msg();
            let m2: Option<i32> = traceforge::recv_msg();
            if m1.is_some() && m2.is_some() {
                assert!(m1.unwrap() == 2 && m2.unwrap() == 1);
            }
        });

        traceforge::send_msg(sh1.thread().id(), rh.thread().id());
        traceforge::send_msg(sh2.thread().id(), rh.thread().id());
    });
}

#[test]
fn s_r_s() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::MO)
                .build(),
            || {
                let sh1 = thread::spawn(move || {
                    let r = traceforge::recv_msg();
                    if r.is_some() {
                        traceforge::send_msg(r.unwrap(), 1);
                    }
                });
                let rh = thread::spawn(move || {
                    let _: Option<i32> = traceforge::recv_msg();
                });
                let sh2 = thread::spawn(move || {
                    let r = traceforge::recv_msg();
                    if r.is_some() {
                        traceforge::send_msg(r.unwrap(), 2);
                    }
                });

                traceforge::send_msg(sh1.thread().id(), rh.thread().id());
                traceforge::send_msg(sh2.thread().id(), rh.thread().id());
            },
        );
        assert_eq!(stats.execs, 2 * 2 + 2 * 2); // + 1);
    }
}

#[test]
fn r_s_s() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::MO)
                .build(),
            || {
                let rh = thread::spawn(move || {
                    let _: Option<i32> = traceforge::recv_msg();
                });
                let sh1 = thread::spawn(move || {
                    let r = traceforge::recv_msg();
                    if r.is_some() {
                        traceforge::send_msg(r.unwrap(), 1);
                    }
                });
                let sh2 = thread::spawn(move || {
                    let r = traceforge::recv_msg();
                    if r.is_some() {
                        traceforge::send_msg(r.unwrap(), 2);
                    }
                });

                traceforge::send_msg(sh1.thread().id(), rh.thread().id());
                traceforge::send_msg(sh2.thread().id(), rh.thread().id());
            },
        );
        assert_eq!(stats.execs, 2 * 2 + 2 * 2); // + 1);
    }
}

#[test]
fn r_ss() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(Config::builder().with_policy(Arbitrary).build(), || {
            let rh = thread::spawn(move || {
                let _: Option<i32> = traceforge::recv_msg();
            });
            let sh = thread::spawn(move || {
                let r = traceforge::recv_msg();
                if r.is_some() {
                    traceforge::send_msg(r.unwrap(), 1);
                    traceforge::send_msg(r.unwrap(), 2);
                }
            });
            traceforge::send_msg(sh.thread().id(), rh.thread().id());
        });
        assert_eq!(stats.execs, 3);
    }
}

#[test]
fn r_ss_assm() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(Config::builder().with_policy(Arbitrary).build(), || {
            let rh = thread::spawn(move || {
                let r: Option<i32> = traceforge::recv_msg();
                traceforge::assume!(r.is_some());
            });
            let sh = thread::spawn(move || {
                let r = traceforge::recv_msg();
                traceforge::assume!(r.is_some());
                traceforge::send_msg(r.unwrap(), 1);
                traceforge::send_msg(r.unwrap(), 2);
            });
            traceforge::send_msg(sh.thread().id(), rh.thread().id());
        });
        assert_eq!(stats.execs, 1);
        assert_eq!(stats.block, 2);
    }
}

#[test]
#[ignore] // Takes 3+ min to complete
fn r_ns_assm() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(Config::builder().with_policy(Arbitrary).build(), || {
            let n = 7;

            let rh = thread::spawn(move || {
                let r: Option<i32> = traceforge::recv_msg();
                traceforge::assume!(r.is_some());
            });
            for i in 0..n {
                let sh = thread::spawn(move || {
                    let r = traceforge::recv_msg();
                    traceforge::assume!(r.is_some());
                    traceforge::send_msg(r.unwrap(), i);
                });
                traceforge::send_msg(sh.thread().id(), rh.thread().id());
            }
        });
        assert_eq!(stats.execs, 5040);
        assert_eq!(stats.block, 22359);
    }
}

#[test]
fn r_s_blk() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(Config::builder().with_policy(Arbitrary).build(), || {
            let rh = thread::spawn(move || {
                let _: i32 = traceforge::recv_msg_block();
            });
            let sh = thread::spawn(move || {
                let r = traceforge::recv_msg_block();
                traceforge::send_msg(r, 1);
            });
            traceforge::send_msg(sh.thread().id(), rh.thread().id());
        });
        assert_eq!(stats.execs, 1);
        assert_eq!(stats.block, 0);
    }
}

#[test]
fn r_rs_s_blk() {
    #[derive(Clone, PartialEq, Eq, Debug)]
    enum MsgType {
        Start(traceforge::thread::ThreadId),
        Int(i32),
    }

    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(Config::builder().with_policy(Arbitrary).build(), || {
            let rh = thread::spawn(move || {
                let _: MsgType = traceforge::recv_msg_block();
            });
            let sh1 = thread::spawn(move || {
                let r = traceforge::recv_msg_block();
                if let MsgType::Start(id) = r {
                    traceforge::send_msg(id, MsgType::Int(42))
                }
            });
            let sh2 = thread::spawn(move || {
                let s1 = traceforge::recv_msg_block();
                if let MsgType::Start(id) = s1 {
                    traceforge::send_msg(id, MsgType::Int(42))
                }
            });
            traceforge::send_msg(sh1.thread().id(), MsgType::Start(rh.thread().id()));
            traceforge::send_msg(sh2.thread().id(), MsgType::Start(sh1.thread().id()));
        });
        assert_eq!(stats.execs, 1);
        assert_eq!(stats.block, 1);
    }
}

#[test]
fn s_s_s() {
    fn send_fun() {
        let s = traceforge::recv_msg_block();
        traceforge::send_msg(s, 42)
    }

    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::MO)
                .build(),
            || {
                let sh1 = thread::spawn(send_fun);
                let sh2 = thread::spawn(send_fun);
                let sh3 = thread::spawn(send_fun);
                traceforge::send_msg(sh1.thread().id(), traceforge::thread::current().id());
                traceforge::send_msg(sh2.thread().id(), traceforge::thread::current().id());
                traceforge::send_msg(sh3.thread().id(), traceforge::thread::current().id());
            },
        );
        assert_eq!(stats.execs, 1); //6);
        assert_eq!(stats.block, 0);
    }
}

#[test]
fn rj_s() {
    #[derive(Clone, PartialEq, Debug)]
    enum Response {
        Peachy,
    }

    // have this verbose (chosen at random) so that we get coverage for verbosity
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_verbose(2)
                .with_dot_out("/tmp/__litmus_dot_out")
                .with_trace_out("/tmp/__litmus_trace_out")
                .build(),
            || {
                let sh = thread::spawn(move || {
                    let pid: thread::ThreadId = traceforge::recv_msg_block();
                    traceforge::send_msg(pid, Response::Peachy);
                    return 42;
                });
                traceforge::send_msg(sh.thread().id(), thread::current().id());
                let res = sh.join();
                assert!(matches!(res, Ok(42)));
                assert!(traceforge::recv_msg_block::<Response>() == Response::Peachy);
            },
        );
        assert_eq!(stats.execs, 1);
        assert_eq!(stats.block, 0);
    }
}

#[test]
fn ssrrr_s() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(Config::builder().with_policy(Arbitrary).build(), || {
            let h1 = thread::spawn(move || {
                traceforge::send_msg(thread::current().id(), 42);
                traceforge::send_msg(thread::current().id(), 43);
                let _m1: i32 = traceforge::recv_msg_block();
                let _m2: i32 = traceforge::recv_msg_block();
                let _m3: i32 = traceforge::recv_msg_block();
            });
            let h2 = thread::spawn(move || {
                let t1: thread::ThreadId = traceforge::recv_msg_block();
                traceforge::send_msg(t1, 17);
            });
            traceforge::send_msg(h2.thread().id(), h1.thread().id());
        });
        assert_eq!(stats.execs, 3);
        assert_eq!(stats.block, 0);
    }
}

#[test]
fn srs_s_rs_nodangling() {
    #[derive(Clone, PartialEq, Debug)]
    enum Message {
        Init(thread::ThreadId),
        Val(i32),
    }

    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_verbose(1)
                .with_policy(Arbitrary)
                .build(),
            || {
                let h1 = thread::spawn(move || {
                    let _m1: Option<Message> = traceforge::recv_msg();
                });
                let h2 = thread::spawn(move || {
                    match traceforge::recv_msg_block() {
                        Message::Init(t1) => {
                            let _m2: Option<Message> = traceforge::recv_msg();
                            traceforge::send_msg(t1, Message::Val(42));
                        }
                        _ => {
                            traceforge::assume!(false);
                        }
                    };
                });
                let h3 = thread::spawn(move || match traceforge::recv_msg_block() {
                    Message::Init(t3) => {
                        traceforge::send_msg(t3, Message::Val(42));
                    }
                    _ => panic!(),
                });
                traceforge::send_msg(h3.thread().id(), Message::Init(h2.thread().id()));
                traceforge::send_msg(h2.thread().id(), Message::Init(h1.thread().id()));
            },
        );
        assert_eq!(stats.execs, 4);
        assert_eq!(stats.block, 1);
    }
}

#[test]
fn r_s_ss() {
    #[derive(Clone, PartialEq, Debug)]
    enum Msg {
        TID(thread::ThreadId),
        Val(i32),
    }

    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::FIFO)
                .build(),
            || {
                let rh = thread::spawn(move || {
                    let _: Option<Msg> = traceforge::recv_msg();
                });
                let sh1 = thread::spawn(move || {
                    let s2 = traceforge::recv_msg_block();
                    match s2 {
                        Msg::TID(s2) => traceforge::send_msg(s2, Msg::Val(1)),
                        _ => panic!(),
                    }
                });
                let sh2 = thread::spawn(move || {
                    let r = traceforge::recv_msg_block();
                    match r {
                        Msg::TID(r) => {
                            traceforge::send_msg(thread::current().id(), Msg::Val(2));
                            traceforge::send_msg(r, Msg::Val(42));
                        }
                        _ => traceforge::assume!(false),
                    }
                });
                traceforge::send_msg(sh1.thread().id(), Msg::TID(sh2.thread().id()));
                traceforge::send_msg(sh2.thread().id(), Msg::TID(rh.thread().id()));
            },
        );
        assert_eq!(stats.execs, 2);
        assert_eq!(stats.block, 1);
    }
}

#[test]
fn sr_s() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(Config::builder().with_policy(Arbitrary).build(), || {
            let srh = thread::spawn(move || {
                traceforge::send_msg(thread::current().id(), 0);
                traceforge::send_msg(thread::current().id(), 1);
                let _m: i32 = traceforge::recv_msg_block();
            });
            let sh = thread::spawn(move || {
                let r = traceforge::recv_msg_block();
                traceforge::send_msg(r, 2);
            });
            traceforge::send_msg(sh.thread().id(), srh.thread().id());
        });
        // assert_eq!(stats.execs, 3); // MO
        assert_eq!(stats.execs, 2); // FIFO
        assert_eq!(stats.block, 0);
    }
}

#[test]
fn r_s_r_ss() {
    for i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_seed(i as u64)
                .build(),
            move || {
                println!("Starting program for {}", i);
                let h1 = thread::spawn(move || {
                    let m: Option<i32> = traceforge::recv_msg();
                    traceforge::assume!(m.is_some() && m.unwrap() == 42);
                });
                let h2 = thread::spawn(move || {
                    let r = traceforge::recv_msg_block();
                    traceforge::send_msg(r, 1);
                });
                let h3 = thread::spawn(move || {
                    let _r: i32 = traceforge::recv_msg_block();
                });
                let h4 = thread::spawn(move || {
                    let r3 = traceforge::recv_msg_block();
                    let r1 = traceforge::recv_msg_block();
                    traceforge::send_msg(r3, 2);
                    traceforge::send_msg(r1, 42);
                });
                traceforge::send_msg(h2.thread().id(), h3.thread().id());
                traceforge::send_msg(h4.thread().id(), h3.thread().id());
                traceforge::send_msg(h4.thread().id(), h1.thread().id());
            },
        );
        assert_eq!(stats.execs, 2);
    }
}

#[test]
fn r_s_r_ss_nonblocking() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(Config::default(), || {
            let h1 = thread::spawn(move || {
                let m: Option<i32> = traceforge::recv_msg();
                traceforge::assume!(m.is_some() && m.unwrap() == 42);
            });
            let h2 = thread::spawn(move || {
                let r = traceforge::recv_msg_block();
                traceforge::send_msg(r, 1);
            });
            let h3 = thread::spawn(move || {
                let _r: Option<i32> = traceforge::recv_msg();
            });
            let h4 = thread::spawn(move || {
                let r3 = traceforge::recv_msg_block();
                let r1 = traceforge::recv_msg_block();
                traceforge::send_msg(r3, 2);
                traceforge::send_msg(r1, 42);
            });
            traceforge::send_msg(h2.thread().id(), h3.thread().id());
            traceforge::send_msg(h4.thread().id(), h3.thread().id());
            traceforge::send_msg(h4.thread().id(), h1.thread().id());
        });
        assert_eq!(stats.execs, 3); // but 4 executions under MO
    }
}

#[test]
fn s_rsrr_sss_coplacing() {
    #[derive(Clone, PartialEq, Debug)]
    enum Msg {
        Val(i32),
        TID(thread::ThreadId),
    }

    for _ in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_verbose(2)
                .with_policy(Arbitrary)
                // .with_seed(15869752320480286691) // debugging failure
                // .with_seed(13781725059417413851)
                .build(),
            || {
                let h1 = thread::spawn(move || {
                    traceforge::send_msg(thread::current().id(), Msg::Val(0));
                    traceforge::recv_msg_block::<Msg>();
                    traceforge::recv_msg_block::<Msg>();
                    traceforge::recv_msg_block::<Msg>();
                });
                let h2 = thread::spawn(move || {
                    let t1 = match traceforge::recv_msg_block() {
                        Msg::TID(t1) => t1,
                        _ => {
                            traceforge::assume!(false);
                            panic!()
                        }
                    };
                    traceforge::send_msg(t1, Msg::Val(42));
                });
                let h3 = thread::spawn(move || {
                    let t1 = match traceforge::recv_msg_block() {
                        Msg::TID(t1) => t1,
                        _ => {
                            traceforge::assume!(false);
                            panic!()
                        }
                    };
                    send_msg(t1, Msg::Val(1));
                    send_msg(t1, Msg::Val(2));
                });
                traceforge::send_msg(h2.thread().id(), Msg::TID(h1.thread().id()));
                traceforge::send_msg(h3.thread().id(), Msg::TID(h1.thread().id()));
            },
        );
        assert_eq!(stats.execs, 12);
    }
}

#[test]
fn s_s_s_wb() {
    #[derive(Clone, PartialEq, Debug)]
    enum Msg {
        Val(i32),
        TID(thread::ThreadId),
    }

    let stats = traceforge::verify(
        Config::builder()
            .with_policy(Arbitrary)
            .with_cons_type(ConsType::FIFO)
            .build(),
        || {
            let h1 = thread::spawn(move || {
                traceforge::send_msg(thread::current().id(), Msg::Val(1));
            });
            let h2 = thread::spawn(move || {
                let t1 = match traceforge::recv_msg_block() {
                    Msg::TID(t1) => t1,
                    _ => {
                        traceforge::assume!(false);
                        panic!()
                    }
                };
                traceforge::send_msg(t1, Msg::Val(2));
            });
            let h3 = thread::spawn(move || {
                let t1 = match traceforge::recv_msg_block() {
                    Msg::TID(t1) => t1,
                    _ => {
                        traceforge::assume!(false);
                        panic!()
                    }
                };
                send_msg(t1, Msg::Val(42));
            });
            traceforge::send_msg(h2.thread().id(), Msg::TID(h1.thread().id()));
            traceforge::send_msg(h3.thread().id(), Msg::TID(h1.thread().id()));
        },
    );
    assert_eq!(stats.execs, 1);
}

#[test]
fn ns_r_wb() {
    #[derive(Clone, PartialEq, Debug)]
    enum Msg {
        Val(u32),
        TID(thread::ThreadId),
    }

    let n: u32 = 4;
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::FIFO)
                .build(),
            move || {
                let mut ns = Vec::new();
                for _i in 0..n {
                    ns.push(thread::spawn(move || {
                        let t4 = match traceforge::recv_msg_block() {
                            Msg::TID(t4) => t4,
                            _ => {
                                traceforge::assume!(false);
                                panic!()
                            }
                        };
                        traceforge::send_msg(t4, Msg::Val(n));
                    }));
                }
                let hr = thread::spawn(move || {
                    let _m1: Msg = traceforge::recv_msg_block();
                });

                for i in 0..n {
                    traceforge::send_msg(ns[i as usize].thread().id(), Msg::TID(hr.thread().id()));
                }
            },
        );
        assert_eq!(stats.execs as u32, n);
    }
}

// The two receives order the read sends before the unread sends, causing a cycle
#[test]
fn test_mailbox() {
    let stats = traceforge::verify(
        Config::builder().with_cons_type(ConsType::Mailbox).build(),
        || {
            let t3 = thread::spawn(move || {
                let a: Option<i32> = recv_msg();
                traceforge::assume!(a.is_some() && a.unwrap() == 1);
            });
            let t4 = thread::spawn(move || {
                let b: Option<i32> = recv_msg();
                traceforge::assume!(b.is_some() && b.unwrap() == 2);
            });
            let t3id = t3.thread().id();
            let t4id = t4.thread().id();
            let _ = thread::spawn(move || {
                send_msg(t4id, 1);
                send_msg(t3id, 1);
            });
            let _ = thread::spawn(move || {
                send_msg(t3id, 2);
                send_msg(t4id, 2);
            });
        },
    );
    assert_eq!(stats.execs, 0);
}

#[test]
fn ss_rr_tag() {
    #[derive(Clone, PartialEq, Debug)]
    enum Msg {
        Val(i32),
        TID(thread::ThreadId),
    }

    for _i in 0..TEST_RUNS {
        for cons in [ConsType::MO, ConsType::FIFO] {
            let stats = traceforge::verify(
                Config::builder()
                    .with_policy(Arbitrary)
                    .with_cons_type(cons)
                    .with_verbose(1)
                    .build(),
                || {
                    let hs = thread::spawn(move || {
                        let tr = match traceforge::recv_msg_block() {
                            Msg::TID(tr) => tr,
                            _ => {
                                panic!()
                            }
                        };
                        traceforge::send_tagged_msg(tr, 1, Msg::Val(1));
                        traceforge::send_tagged_msg(tr, 2, Msg::Val(2));
                    });
                    let hr = thread::spawn(move || {
                        let v1 = match traceforge::recv_tagged_msg_block(move |_, t| {
                            t.is_some() && t.unwrap() == 2
                        }) {
                            Msg::Val(v1) => v1,
                            _ => {
                                panic!()
                            }
                        };
                        let v2 = match traceforge::recv_tagged_msg_block(move |_, t| {
                            t.is_some() && t.unwrap() == 1
                        }) {
                            Msg::Val(v2) => v2,
                            _ => {
                                panic!()
                            }
                        };
                        assert!(v1 == 2 && v2 == 1);
                    });
                    traceforge::send_msg(hs.thread().id(), Msg::TID(hr.thread().id()));
                },
            );
            assert_eq!(stats.execs, 1);
        }
    }
}

#[test]
fn ss_rr_tag2() {
    #[derive(Clone, PartialEq, Debug)]
    enum Msg {
        Val(i32),
        TID(thread::ThreadId),
    }

    for _i in 0..TEST_RUNS {
        for cons in [ConsType::MO, ConsType::FIFO] {
            let stats = traceforge::verify(
                Config::builder()
                    .with_policy(Arbitrary)
                    .with_cons_type(cons)
                    .with_verbose(1)
                    .build(),
                || {
                    let hs = thread::spawn(move || {
                        let tr = match traceforge::recv_msg_block() {
                            Msg::TID(tr) => tr,
                            _ => {
                                panic!()
                            }
                        };
                        traceforge::send_tagged_msg(tr, 1, Msg::Val(1));
                        traceforge::send_tagged_msg(tr, 2, Msg::Val(2));
                    });
                    let hr = thread::spawn(move || {
                        let v1 = match traceforge::recv_tagged_msg_block(move |_, t| {
                            t.is_some() && t.unwrap() == 1
                        }) {
                            Msg::Val(v1) => v1,
                            _ => {
                                panic!()
                            }
                        };
                        let v2 = match traceforge::recv_tagged_msg_block(move |_, t| {
                            t.is_some() && t.unwrap() == 2
                        }) {
                            Msg::Val(v2) => v2,
                            _ => {
                                panic!()
                            }
                        };
                        assert!(v1 == 1 && v2 == 2);
                    });
                    traceforge::send_msg(hs.thread().id(), Msg::TID(hr.thread().id()));
                },
            );
            assert_eq!(stats.execs, 1);
        }
    }
}

#[test]
fn ss_rr_tag_nb() {
    #[derive(Clone, PartialEq, Debug)]
    enum Msg {
        Val(i32),
        TID(thread::ThreadId),
    }

    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::FIFO)
                .with_verbose(1)
                .build(),
            || {
                let hs = thread::spawn(move || {
                    let tr = match traceforge::recv_msg_block() {
                        Msg::TID(tr) => tr,
                        _ => {
                            panic!()
                        }
                    };
                    traceforge::send_tagged_msg(tr, 1, Msg::Val(1));
                    traceforge::send_tagged_msg(tr, 2, Msg::Val(2));
                });
                let hr = thread::spawn(move || {
                    let v1 = match traceforge::recv_tagged_msg(move |_, t| {
                        t.is_some() && t.unwrap() == 1
                    }) {
                        Some(Msg::Val(v1)) => v1,
                        None => 100,
                        _ => {
                            panic!()
                        }
                    };
                    let v2 = match traceforge::recv_tagged_msg(move |_, t| {
                        t.is_some() && t.unwrap() == 2
                    }) {
                        Some(Msg::Val(v2)) => v2,
                        None => 200,
                        _ => {
                            panic!()
                        }
                    };
                    assert!((v1 == 100 || v1 == 1) && (v2 == 200 || v2 == 2));
                });
                traceforge::send_msg(hs.thread().id(), Msg::TID(hr.thread().id()));
            },
        );
        assert_eq!(stats.execs, 4);
        assert_eq!(stats.block, 0);
    }
}

#[test]
fn ss_rr_tag3() {
    #[derive(Clone, PartialEq, Debug)]
    enum Msg {
        Val(i32),
        TID(thread::ThreadId),
    }

    for _i in 0..TEST_RUNS {
        for cons in [ConsType::MO, ConsType::FIFO] {
            let stats = traceforge::verify(
                Config::builder()
                    .with_policy(Arbitrary)
                    .with_cons_type(cons)
                    .with_verbose(1)
                    .build(),
                || {
                    let hs = thread::spawn(move || {
                        let tr = match traceforge::recv_msg_block() {
                            Msg::TID(tr) => tr,
                            _ => {
                                panic!()
                            }
                        };
                        traceforge::send_tagged_msg(tr, 1, Msg::Val(1));
                        traceforge::send_tagged_msg(tr, 2, Msg::Val(2));
                    });
                    let hr = thread::spawn(move || {
                        let v1 = match traceforge::recv_tagged_msg_block(move |_, t| {
                            t.is_some() && t.unwrap() == 2
                        }) {
                            Msg::Val(v1) => v1,
                            _ => {
                                panic!()
                            }
                        };
                        let v2 = match traceforge::recv_tagged_msg_block(move |_, t| {
                            t.is_some() && t.unwrap() == 1
                        }) {
                            Msg::Val(v2) => v2,
                            _ => {
                                panic!()
                            }
                        };
                        assert!(v1 == 2 && v2 == 1);
                    });
                    traceforge::send_msg(hs.thread().id(), Msg::TID(hr.thread().id()));
                },
            );
            assert_eq!(stats.execs, 1);
        }
    }
}

#[test]
fn ss_rr_ss_tag() {
    #[derive(Clone, PartialEq, Debug)]
    enum Msg {
        Val(i32),
        TID(thread::ThreadId),
    }

    for _i in 0..TEST_RUNS {
        for cons in [ConsType::FIFO, ConsType::MO] {
            let stats = traceforge::verify(
                Config::builder()
                    .with_policy(Arbitrary)
                    .with_cons_type(cons)
                    .with_verbose(2)
                    .build(),
                || {
                    let hprev = thread::spawn(move || {
                        let tr = match traceforge::recv_msg_block() {
                            Msg::TID(tr) => tr,
                            _ => {
                                panic!()
                            }
                        };
                        traceforge::send_tagged_msg(tr, 2, Msg::Val(2));
                        traceforge::send_tagged_msg(tr, 1, Msg::Val(1));
                    });
                    let hr = thread::spawn(move || {
                        let _v1 = match traceforge::recv_tagged_msg_block(|_, t| {
                            t.is_some() && t.unwrap() == 2
                        }) {
                            Msg::Val(v1) => v1,
                            _ => {
                                panic!()
                            }
                        };
                        let _v2 = match traceforge::recv_tagged_msg_block(|_, t| {
                            t.is_some() && t.unwrap() == 1
                        }) {
                            Msg::Val(v2) => v2,
                            _ => {
                                panic!()
                            }
                        };
                        // assert!(v1 == 2 && v2 == 1);
                    });
                    let hs = thread::spawn(move || {
                        let tr = match traceforge::recv_msg_block() {
                            Msg::TID(tr) => tr,
                            _ => {
                                panic!()
                            }
                        };
                        traceforge::send_tagged_msg(tr, 1, Msg::Val(1));
                        traceforge::send_msg(tr, Msg::Val(2));
                    });
                    traceforge::send_msg(hprev.thread().id(), Msg::TID(hr.thread().id()));
                    traceforge::send_msg(hs.thread().id(), Msg::TID(hr.thread().id()));
                },
            );
            assert_eq!(stats.execs, 2);
        }
    }
}

#[test]
fn ns_r_tag() {
    #[derive(Clone, PartialEq, Debug)]
    enum Msg {
        Val(u32),
        TID(thread::ThreadId),
    }

    let n: u32 = 4;
    for _i in 0..TEST_RUNS {
        for blocking in [false, true] {
            let stats = traceforge::verify(
                Config::builder()
                    .with_policy(Arbitrary)
                    .with_cons_type(ConsType::FIFO)
                    .with_verbose(1)
                    .build(),
                move || {
                    let mut ns = Vec::new();
                    for i in 0..n {
                        ns.push(thread::spawn(move || {
                            let t4 = match traceforge::recv_msg_block() {
                                Msg::TID(t4) => t4,
                                _ => {
                                    panic!()
                                }
                            };
                            let val = i + 101;
                            traceforge::send_tagged_msg(t4, val, Msg::Val(val));
                        }));
                    }
                    let t1 = ns[0].thread().id();
                    let hr = thread::spawn(move || {
                        if blocking {
                            let _m1: Msg = recv_tagged_msg_block(move |tid, _| tid == t1);
                        } else {
                            let _: Option<Msg> = recv_tagged_msg(move |tid, _| tid == t1);
                        }
                    });

                    for i in 0..n {
                        traceforge::send_msg(
                            ns[i as usize].thread().id(),
                            Msg::TID(hr.thread().id()),
                        );
                    }
                },
            );
            assert_eq!(stats.execs as u32, if blocking { 1 } else { 2 });
        }
    }
}

#[test]
fn nested_spawn() {
    let stats = traceforge::verify(
        Config::builder().with_policy(LTR).with_verbose(1).build(),
        || {
            let t1 = thread::spawn(|| {
                let r: u32 = recv_msg_block();
                let mut zero = false;
                if r == 0 {
                    zero = true;
                    let _: u32 = recv_msg_block();
                }
                //let myid = thread::current().id();
                let _ = thread::spawn(move || {
                    //send_msg(myid, 2u32);
                });
                //let _: u32 = recv_msg_block();
                if zero {
                    let _: u32 = recv_msg_block();
                }
            });
            let t1id = t1.thread().id();
            let _t2 = thread::spawn(move || {
                send_msg(t1id, 0u32);
            });
            //let t1id = t1.thread().id();
            let _t3 = thread::spawn(move || {
                send_msg(t1id, 1u32);
            });
        },
    );
    assert_eq!(stats.execs, 1);
}

#[test]
fn nested_spawn2() {
    let stats = traceforge::verify(
        Config::builder()
            .with_policy(crate::SchedulePolicy::Arbitrary)
            .with_seed(16421934074137481575)
            .with_verbose(100)
            .build(),
        || {
            let _t1 = thread::spawn(|| {
                let _t4 = thread::spawn(move || {});
            });

            let t2 = thread::spawn(|| {
                let mtid: ThreadId = recv_msg_block();
                send_msg(mtid, 1u32);
            });

            let tid2 = t2.thread().id();
            send_msg(tid2, thread::current().id());

            send_msg(thread::current().id(), 0u32);
            let _: u32 = recv_msg_block();
        },
    );
    assert_eq!(stats.execs, 2);
}

#[test]
fn nested_spawn3() {
    let stats = traceforge::verify(
        Config::builder()
            .with_policy(crate::SchedulePolicy::LTR)
            .with_verbose(100)
            .build(),
        || {
            let t1 = thread::spawn(|| {
                let mtid: ThreadId = recv_msg_block();

                let t2 = thread::spawn(|| {
                    let mtid: ThreadId = recv_msg_block();
                    send_msg(mtid, 1u32);
                });

                let tid2 = t2.thread().id();
                send_msg(tid2, mtid);
            });

            let tid1 = t1.thread().id();
            send_msg(tid1, thread::current().id());

            send_msg(thread::current().id(), 0u32);
            let x: u32 = recv_msg_block();

            if x == 0 {
                let _t3 = thread::spawn(move || {});
            }
        },
    );
    assert_eq!(stats.execs, 2);
}

// Testcase that shows how a more arbitrary tiebreaking that consults the stamp
// order would not be correct.
// This test would have more than one execution if the maximal send for the
// main thread's receive wrt to the prefix of send(3) returns
// the stamp-minimal between send(1) and send(2), instead of always
// choosing send(0).
#[test]
fn weird_stamp_tiebreak() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                // Don't force reading 0
                .with_cons_type(ConsType::Bag)
                // We want the main -> t1 -> t2 scheduling
                .with_policy(crate::SchedulePolicy::Arbitrary)
                .build(),
            || {
                // So that the receive doesn't block when added
                send_msg(main_thread_id(), 0);
                let t2 = thread::spawn(|| {
                    send_msg(main_thread_id(), 2);
                    let b: Option<i32> = recv_msg();
                    if b.is_some() && b.unwrap() == 1 {
                        send_msg(main_thread_id(), 3);
                    }
                });
                let _t1 = thread::spawn(move || {
                    send_msg(main_thread_id(), 1);
                    send_msg(t2.thread().id(), 1);
                });
                // Blocking to get non-trivial maximality check
                assume!(recv_msg_block::<i32>() == 3);
            },
        );
        assert_eq!(stats.execs, 1);
    }
}

// Testcase that ensures that the revisitng optimization
// (sort by stamp and abort early) is done in the
// correct order
#[test]
fn revisit_opt() {
    for _ in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(SchedulePolicy::Arbitrary)
                .with_cons_type(ConsType::Bag)
                .build(),
            || {
                let t1 = thread::spawn(|| {
                    let _: Option<i32> = recv_msg();
                    let _: Option<i32> = recv_msg();
                });
                let t1id = t1.thread().id();
                let x = t1id.clone();
                let y = t1id.clone();
                thread::spawn(move || {
                    send_msg(x, 1);
                });
                thread::spawn(move || {
                    send_msg(y, 2);
                });
            },
        );
        assert_eq!(stats.execs, 7);
    }
}

// Testcase that ensures that the filtering of revisits is not
// too aggressive, possibly missing a revisit
#[test]
fn revisit_opt_2() {
    for _ in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::FIFO)
                .with_verbose(1)
                .build(),
            || {
                let t1 = thread::spawn(|| {
                    let _: Option<i32> = recv_tagged_msg(|_, t| {
                        return t.is_some() && t.unwrap() == 2;
                    });
                    let _: i32 = recv_msg_block();
                });
                let t1id = t1.thread().id();
                let _t2 = thread::spawn(move || {
                    send_tagged_msg(t1id, 1, 1);
                    send_tagged_msg(t1id, 2, 2);
                });
            },
        );
        assert_eq!(stats.execs, 2);
    }
}

// Main thread orders the two sends by receiveing them (mailbox semantics).
// The receive can timeout even if said ordering transitively orders a send before it.
#[test]
fn spurious_timeout_mailbox() {
    let stats = traceforge::verify(
        Config::builder()
            .with_policy(LTR)
            .with_cons_type(ConsType::Mailbox)
            .build(),
        || {
            let t1 = thread::spawn(move || {
                send_msg(main_thread_id(), 2);
                assume!(recv_msg::<i32>().is_none());
            });
            let _t2 = thread::spawn(move || {
                send_msg(t1.thread().id(), 1);
                send_msg(main_thread_id(), 1);
            });
            assume!(recv_msg_block::<i32>() == 1);
            assume!(recv_msg_block::<i32>() == 2);
        },
    );
    assert_eq!(stats.execs, 1);
}
