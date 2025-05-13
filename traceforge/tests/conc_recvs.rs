use traceforge::{channel::Builder, loc::CommunicationModel::*, thread, Config};

// In the presence of concurrent receives and consistency stronger than NoOrder,
// the model is not prefix-closed and one consistent execution is missed.
#[should_panic(expected = "Detected concurrent receives")]
#[test]
fn not_prefix_closed() {
    let stats = traceforge::verify(Config::builder().build(), || {
        let ch1 = Builder::new().with_comm(LocalOrder).build();
        let ch2 = ch1.clone();
        ch1.0.send_msg(1);
        ch1.0.send_msg(2);
        let _t1 = thread::spawn(move || {
            traceforge::assume!(ch1.1.recv_msg() == Some(2));
        });
        let _t2 = thread::spawn(move || {
            traceforge::assume!(ch2.1.recv_msg() == Some(1));
        });
    });
    assert_eq!(stats.execs, 1);
}

// Receive->receive revisits won't work out of the box:
// two receives would have to revisit the first one,
// but once the first revisit, the second cannot.
#[should_panic(expected = "Detected concurrent receives")]
#[test]
fn not_prefix_closed_extra() {
    let stats = traceforge::verify(Config::builder().build(), || {
        let ch1 = Builder::new().with_comm(LocalOrder).build();
        let ch2 = ch1.clone();
        let ch3 = ch1.clone();
        ch1.0.send_msg(1);
        ch1.0.send_msg(2);
        ch1.0.send_msg(3);
        let _t1 = thread::spawn(move || {
            traceforge::assume!(ch1.1.recv_msg() == Some(3));
        });
        let _t2 = thread::spawn(move || {
            ch2.1.recv_msg();
        });
        let _t3 = thread::spawn(move || {
            ch3.1.recv_msg();
        });
    });
    assert!(stats.execs >= 1);
}

// In the presence of blocking concurrent receives, the model is not extensible.
// missing one consistent execution.
#[should_panic(expected = "Detected concurrent receives")]
#[test]
fn not_extensible() {
    let stats = traceforge::verify(Config::builder().build(), || {
        let ch1 = Builder::new().with_comm(NoOrder).build();
        let ch2 = ch1.clone();
        ch1.0.send_msg(1);
        let _t1 = thread::spawn(move || {
            ch1.1.recv_msg_block();
            traceforge::assume!(ch1.1.recv_msg_block() == 2);
        });
        let _t2 = thread::spawn(move || {
            ch2.1.recv_msg_block();
            ch2.0.send_msg(2);
        });
    });
    assert_eq!(stats.execs, 1);
}

// One blocked execution is missed.
#[should_panic(expected = "Detected concurrent receives")]
#[test]
fn not_extensible_2() {
    let stats = traceforge::verify(Config::builder().build(), || {
        let ch1 = Builder::new().with_comm(NoOrder).build();
        let ch2 = ch1.clone();
        ch1.0.send_msg(1);
        let _t2 = thread::spawn(move || ch1.1.recv_msg_block());
        let _t2 = thread::spawn(move || ch2.1.recv_msg_block());
    });
    assert_eq!((stats.execs, stats.block), (0, 2));
}
