use traceforge::thread;
use traceforge::*;
use thread::ThreadId;
use SchedulePolicy::*;

const TEST_RUNS: u32 = 10;

// the "sem1/2/3_comp_" tests check the number of executions of program1 and program2 when strenghening the semantics

// Besides a first part where the main thread sends process ids to everyone (each receiver must acknowledge every message received so the order of thread id sends is preserved),
//      - actor_a sends a message to actor_m and then a message to actor_b
//      - actor_b receives the message from actor_a and then sends a message to actor_m
//      - main sends two messages to actor_m
fn program1() {
    let actor_m = thread::spawn(move || {
        // receiving the id of the main thread and sending an ACK message
        let tmain: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);

        // receives that determine the number of executions
        // under causal delivery and mailbox, all prefixes of length 3 of interleavings between [1;2] and [3;4] (messages 3 is causally-before 4)
        // under fifo, all prefixes of length 3 of interleavings between [1;2], [3], and [4]
        // under bag, all orders between 3 out of the 4 values {1,2,3,4}
        let _: i32 = traceforge::recv_msg_block();
        let _: i32 = traceforge::recv_msg_block();
        let _: i32 = traceforge::recv_msg_block();
    });
    let actor_a = thread::spawn(move || {
        // receiving the ids of the other threads, sending an ACK message for each receive
        // the last receive ensures that actor_a does not start sending important messages before actor_b received all thread ids
        let tmain: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let m: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let b: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let _: i32 = traceforge::recv_msg_block();

        // important messages
        traceforge::send_msg(m, 3);
        traceforge::send_msg(b, 5);
    });
    let actor_b = thread::spawn(move || {
        // receiving the ids of the other threads and sending an ACK message for each receive
        let tmain: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let m: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let _a: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);

        // important messages
        let _: i32 = traceforge::recv_msg_block();
        traceforge::send_msg(m, 4);
    });

    // sending thread ids: the additional sends/receives are needed in order to "separate" the phase of exchanging thread ids from the "important" part
    traceforge::send_msg(actor_m.thread().id(), thread::current().id());
    let _: i32 = traceforge::recv_msg_block();

    traceforge::send_msg(actor_a.thread().id(), thread::current().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_a.thread().id(), actor_m.thread().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_a.thread().id(), actor_b.thread().id());
    let _: i32 = traceforge::recv_msg_block();

    traceforge::send_msg(actor_b.thread().id(), thread::current().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_b.thread().id(), actor_m.thread().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_b.thread().id(), actor_a.thread().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_a.thread().id(), 0);

    // important messages
    traceforge::send_msg(actor_m.thread().id(), 1);
    traceforge::send_msg(actor_m.thread().id(), 2);
}

#[test]
fn sem1_comp_bag() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::Bag)
                .build(),
            program1,
        );
        println!("Number of executions explored {}", stats.execs);
        println!("Number of blocked executions explored {}", stats.block);
        assert_eq!(stats.execs, 24); // all orders between 3 out of the 4 values {1,2,3,4}
    }
}

#[test]
fn sem1_comp_fifo() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::FIFO)
                .build(),
            program1,
        );
        println!("Number of executions explored {}", stats.execs);
        println!("Number of blocked executions explored {}", stats.block);
        assert_eq!(stats.execs, 12); // all prefixes of length 3 of interleavings between [1;2], [3], and [4]
    }
}

#[test]
fn sem1_comp_causal() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::Causal)
                .build(),
            program1,
        );
        println!("Number of executions explored {}", stats.execs);
        println!("Number of blocked executions explored {}", stats.block);
        assert_eq!(stats.execs, 6); // all prefixes of length 3 of interleavings between [1;2] and [3;4] (messages 3 is causally-before 4)
    }
}

#[test]
fn sem1_comp_mailbox() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::Mailbox)
                .build(),
            program1,
        );
        println!("Number of executions explored {}", stats.execs);
        println!("Number of blocked executions explored {}", stats.block);
        assert_eq!(stats.execs, 6); // all prefixes of length 3 of interleavings between [1;2] and [3;4] (messages 3 is causally-before 4)
    }
}

// Like program1, except that the messages sent to actor_m by actor_a and actor_b are not anymore causally related, i.e., actor_a sends no message to actor_b
fn program2() {
    let actor_m = thread::spawn(move || {
        // receiving the id of the main thread and sending an ACK message
        let tmain: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);

        // receives that determine the number of executions
        // under mailbox, causal delivery, and fifo, all prefixes of length 3 of interleavings between [1;2], [3], and [4]
        // under bag, all orders between 3 out of the 4 values {1,2,3,4}
        let _: i32 = traceforge::recv_msg_block();
        let _: i32 = traceforge::recv_msg_block();
        let _: i32 = traceforge::recv_msg_block();
    });
    let actor_a = thread::spawn(move || {
        // receiving the ids of the other threads, sending an ACK message for each receive
        // the last receive ensures that actor_a does not start sending important messages before actor_b received all thread ids
        let tmain: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let m: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let _b: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let _: i32 = traceforge::recv_msg_block();

        // important messages
        traceforge::send_msg(m, 3);
    });
    let actor_b = thread::spawn(move || {
        // receiving the ids of the other threads and sending an ACK message for each receive
        let tmain: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let m: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let _a: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);

        // important messages
        traceforge::send_msg(m, 4);
    });

    // sending thread ids: the additional sends/receives are needed in order to "separate" the phase of exchanging thread ids from the "important" part
    traceforge::send_msg(actor_m.thread().id(), thread::current().id());
    let _: i32 = traceforge::recv_msg_block();

    traceforge::send_msg(actor_a.thread().id(), thread::current().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_a.thread().id(), actor_m.thread().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_a.thread().id(), actor_b.thread().id());
    let _: i32 = traceforge::recv_msg_block();

    traceforge::send_msg(actor_b.thread().id(), thread::current().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_b.thread().id(), actor_m.thread().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_b.thread().id(), actor_a.thread().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_a.thread().id(), 0);

    // important messages
    traceforge::send_msg(actor_m.thread().id(), 1);
    traceforge::send_msg(actor_m.thread().id(), 2);
}

#[test]
fn sem2_comp_bag() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::Bag)
                .build(),
            program2,
        );
        println!("Number of executions explored {}", stats.execs);
        println!("Number of blocked executions explored {}", stats.block);
        assert_eq!(stats.execs, 24); // all orders between 3 out of the 4 values {1,2,3,4}
    }
}

#[test]
fn sem2_comp_fifo() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::FIFO)
                .build(),
            program2,
        );
        println!("Number of executions explored {}", stats.execs);
        println!("Number of blocked executions explored {}", stats.block);
        assert_eq!(stats.execs, 12); // all prefixes of length 3 of interleavings between [1;2], [3], and [4]
    }
}

#[test]
fn sem2_comp_causal() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::Causal)
                .build(),
            program2,
        );
        println!("Number of executions explored {}", stats.execs);
        println!("Number of blocked executions explored {}", stats.block);
        assert_eq!(stats.execs, 12); // all prefixes of length 3 of interleavings between [1;2], [3], and [4]
    }
}

#[test]
fn sem2_comp_mailbox() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::Causal)
                .build(),
            program2,
        );
        println!("Number of executions explored {}", stats.execs);
        println!("Number of blocked executions explored {}", stats.block);
        assert_eq!(stats.execs, 12); // all prefixes of length 3 of interleavings between [1;2], [3], and [4]
    }
}

// the "sem3_comp..." tests check that program3 has the same number of executions under Mailbox, Causal, FIFO, and more executions under Bag

// Besides a first part where the main thread sends process ids to everyone,
//      - actor_a sends a message to actor_m and then a message to actor_b
//      - actor_b receives the message from actor_a and then sends a message to actor_m
//      - main sends two messages to actor_m
// Under causal delivery, actor_m should receive the message from actor_a before the one from actor_b, interleaved with the two messages from main
// Under peer-to-peer, any order is possible between the messages from actor_a and actor_b, and hence, the number of executions is larger
fn program3() {
    let actor_a = thread::spawn(move || {
        // receiving the messages from the main thread
        let _: i32 = traceforge::recv_msg_block();
        let _: i32 = traceforge::recv_msg_block();
        let _: i32 = traceforge::recv_msg_block();
    });

    // important messages
    traceforge::send_msg(actor_a.thread().id(), 1);
    traceforge::send_msg(actor_a.thread().id(), 2);
    traceforge::send_msg(actor_a.thread().id(), 3);
}

#[test]
fn sem3_comp_bag() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::Bag)
                .build(),
            program3,
        );
        println!("Number of executions explored {}", stats.execs);
        println!("Number of blocked executions explored {}", stats.block);
        assert_eq!(stats.execs, 6); // all orders between 1, 2, 3
    }
}

#[test]
fn sem3_comp_fifo() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::FIFO)
                .build(),
            program3,
        );
        println!("Number of executions explored {}", stats.execs);
        println!("Number of blocked executions explored {}", stats.block);
        assert_eq!(stats.execs, 1); // exactly the order 1,2,3
    }
}

#[test]
fn sem3_comp_causal() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::Causal)
                .build(),
            program3,
        );
        println!("Number of executions explored {}", stats.execs);
        println!("Number of blocked executions explored {}", stats.block);
        assert_eq!(stats.execs, 1); // exactly the order 1,2,3
    }
}

#[test]
fn sem3_comp_mailbox() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::Mailbox)
                .build(),
            program3,
        );
        println!("Number of executions explored {}", stats.execs);
        println!("Number of blocked executions explored {}", stats.block);
        assert_eq!(stats.execs, 1); // exactly the order 1,2,3
    }
}

// This reproduces the non mailbox execution from Fig 7(a) in "A Partial Order View of Message-Passing Communication Models" (POPL'23)
// Besides a first part where the main thread sends process ids to everyone (each receiver must acknowledge every message received so these messages do not mix with the important ones),
//      - main sends a message to actor_m and then one to actor_a
//      - actor_a receives two messages
//      - actor_b sends a message to actor_a and then one to actor_m
//      - actor_m receives two messages
// Under causal delivery, both actor_a and actor_m can receive messages in any order. 4 executions in total.
// Under mailbox, actor_a cannot receive from main and then actor_b while actor_m receives from actor_b and then main. 3 executions in total
fn program4() {
    let actor_m = thread::spawn(move || {
        // receiving the id of the main thread and sending an ACK message
        let tmain: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);

        // important receives
        let _: i32 = traceforge::recv_msg_block();
        let _: i32 = traceforge::recv_msg_block();
    });
    let actor_a = thread::spawn(move || {
        // receiving the ids of the other threads, sending an ACK message for each message
        let tmain: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let _m: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let _b: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);

        // important receives
        let _: i32 = traceforge::recv_msg_block();
        let _: i32 = traceforge::recv_msg_block();
    });
    let actor_b = thread::spawn(move || {
        // receiving the ids of the other threads and sending an ACK message
        let tmain: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let m: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let a: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);

        // important messages
        traceforge::send_msg(a, 3);
        traceforge::send_msg(m, 4);
    });

    // sending thread ids: the additional sends/receives are needed in order to "separate" the phase of exchanging thread ids from the "important" part
    traceforge::send_msg(actor_m.thread().id(), thread::current().id());
    let _: i32 = traceforge::recv_msg_block();

    traceforge::send_msg(actor_a.thread().id(), thread::current().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_a.thread().id(), actor_m.thread().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_a.thread().id(), actor_b.thread().id());
    let _: i32 = traceforge::recv_msg_block();

    traceforge::send_msg(actor_b.thread().id(), thread::current().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_b.thread().id(), actor_m.thread().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_b.thread().id(), actor_a.thread().id());
    let _: i32 = traceforge::recv_msg_block();

    // important messages
    traceforge::send_msg(actor_m.thread().id(), 1);
    traceforge::send_msg(actor_a.thread().id(), 2);
}

#[test]
fn mailbox1_mb() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::Mailbox)
                .build(),
            program4,
        );
        println!("Number of executions explored {}", stats.execs);
        println!("Number of blocked executions explored {}", stats.block);
        assert_eq!(stats.execs, 3);
    }
}

#[test]
fn mailbox1_cd() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::Causal)
                .build(),
            program4,
        );
        println!("Number of executions explored {}", stats.execs);
        println!("Number of blocked executions explored {}", stats.block);
        assert_eq!(stats.execs, 4);
    }
}

// Like program4, but with an assertion that checks a property which should be satisfied under mailbox but not causal.
// actor_a sends a message to main if it receives from main before actor_b
// actor_m sends a message to main if it receives from actor_b before main
// if main receives both messages if fails the assertion
fn program5() {
    let actor_m = thread::spawn(move || {
        // receiving the id of the main thread and sending an ACK message
        let tmain: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);

        // important receives
        let z: i32 = traceforge::recv_msg_block();
        let t: i32 = traceforge::recv_msg_block();

        if z > t {
            traceforge::send_msg(tmain, 5);
        }
    });
    let actor_a = thread::spawn(move || {
        // receiving the ids of the other threads, sending an ACK for each message
        let tmain: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let _m: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let _b: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);

        // important receives
        let x: i32 = traceforge::recv_msg_block();
        let y: i32 = traceforge::recv_msg_block();
        if x < y {
            traceforge::send_msg(tmain, 5);
        }
    });
    let actor_b = thread::spawn(move || {
        // receiving the ids of the other threads and sending an ACK for each message
        let tmain: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let m: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);
        let a: ThreadId = traceforge::recv_msg_block();
        traceforge::send_msg(tmain, 0);

        // important messages
        traceforge::send_msg(a, 3);
        traceforge::send_msg(m, 4);
    });

    // sending thread ids: the additional sends/receives are needed in order to "separate" the phase of exchanging thread ids from the "important" part
    traceforge::send_msg(actor_m.thread().id(), thread::current().id());
    let _: i32 = traceforge::recv_msg_block();

    traceforge::send_msg(actor_a.thread().id(), thread::current().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_a.thread().id(), actor_m.thread().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_a.thread().id(), actor_b.thread().id());
    let _: i32 = traceforge::recv_msg_block();

    traceforge::send_msg(actor_b.thread().id(), thread::current().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_b.thread().id(), actor_m.thread().id());
    let _: i32 = traceforge::recv_msg_block();
    traceforge::send_msg(actor_b.thread().id(), actor_a.thread().id());
    let _: i32 = traceforge::recv_msg_block();

    // important messages
    traceforge::send_msg(actor_m.thread().id(), 1);
    traceforge::send_msg(actor_a.thread().id(), 2);

    let _: i32 = traceforge::recv_msg_block();
    let _: i32 = traceforge::recv_msg_block();
    traceforge::assert(false);
}

#[test]
fn mailbox2_mb() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::Mailbox)
                .with_verbose(1)
                .build(),
            program5,
        );
        println!("Number of executions explored {}", stats.execs);
        println!("Number of blocked executions explored {}", stats.block);
        // now the main can block and therefore we count both terminated and blocked executions
        assert_eq!(stats.execs + stats.block, 3); // the number of executions remains the same as for program1
    }
}

#[test]
#[should_panic(expected = "assertion failed")]
fn mailbox2_cd() {
    for _i in 0..TEST_RUNS {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(ConsType::Causal)
                .build(),
            program5,
        );
        println!("Number of executions explored {}", stats.execs);
        println!("Number of blocked executions explored {}", stats.block);
        // now the main can block and therefore we count both terminated and blocked executions
        assert_eq!(stats.execs + stats.block, 5); // for the non-mailbox execution, main can receive the messages from actor_a and actor_m in two possible orders (the number of execution grows with 1)
    }
}
