use traceforge::*;
use channel::Builder;
use loc::CommunicationModel;

// Interesting litmus tests that showcase
// the behavior of channels in different scenarios.

// A receive can timeout, even if there is a send in the same thread
#[test]
fn spurious_timeout() {
    let stats = traceforge::verify(Config::builder().build(), || {
        let chan = Builder::new()
            .with_comm(CommunicationModel::NoOrder)
            .build();
        chan.0.send_msg(1);
        let _ = chan.1.recv_msg();
    });
    assert_eq!(stats.execs, 2);
}

// Tagged receives do not synchronize: even under the strongest semantics,
// reading from the po-last of two sends does not force a later receive
// to observe the first send.
#[test]
fn ss_rr_no_tag_sync() {
    let stats = traceforge::verify(
        Config::builder()
            .with_cons_type(ConsType::Mailbox)
            .with_verbose(2)
            .build(),
        || {
            let (sender, receiver) = Builder::new().build();
            let _ = thread::spawn(move || {
                let _ = receiver.recv_tagged_msg_block(|x| x == Some(2));
                let _ = receiver.recv_msg();
            });

            sender.send_msg(1);
            sender.send_tagged_msg(2, 2);
        },
    );
    assert_eq!(stats.execs, 2);
}

// Concurrent receives are not allowed
#[should_panic(expected = "Detected concurrent receives")]
#[test]
fn concurrent_receives_fail() {
    let stats = traceforge::verify(
        Config::builder()
            .with_policy(traceforge::SchedulePolicy::Arbitrary)
            .build(),
        move || {
            let (sender1, receiver1) = Builder::new()
                .with_comm(CommunicationModel::LocalOrder)
                .with_name(42)
                .build();
            let (sender2, receiver2) = Builder::new()
                .with_comm(CommunicationModel::LocalOrder)
                .with_name(42)
                .build();
            let _ = thread::spawn(move || {
                let _ = receiver1.recv_msg();
            });
            let _ = thread::spawn(move || {
                let _ = receiver2.recv_msg();
            });
            sender1.send_msg(1);
            sender2.send_msg(2);
        },
    );
    // (*,*), (*,1), (*,1), (1,2), (2,1)
    assert_eq!(stats.execs, 5);
}

// channels with the same identifier but different models are *not* the same
#[should_panic(expected = "Detected channel with identifier")]
#[ignore = "WIP"]
#[test]
fn channel_model_id() {
    let stats = traceforge::verify(Config::builder().build(), move || {
        let ch1 = Builder::new()
            .with_name(42)
            .with_comm(CommunicationModel::NoOrder)
            .build();
        let ch2 = Builder::new()
            .with_name(42)
            .with_comm(CommunicationModel::LocalOrder)
            .build();
        ch1.0.send_msg(1);
        let () = ch2.1.recv_msg_block();
    });
    assert_eq!(stats.execs, 0);
    assert_eq!(stats.block, 1);
}

fn unique_channel<T: msg::Message + Clone + 'static>() -> channel::Channel<T> {
    Builder::new().build()
}

#[test]
fn local_unique_channel() {
    traceforge::verify(Config::builder().build(), move || {
        let ch1 = unique_channel();
        let ch2 = unique_channel::<()>();
        ch1.0.send_msg(());
        assert!(ch2.1.recv_msg().is_none())
    });
}

#[test]
fn global_unique_channel() {
    traceforge::verify(Config::builder().build(), move || {
        let (sender, receiver) = unique_channel();
        let senderp = sender.clone();
        let _ = thread::spawn(move || {
            sender.send_msg(unique_channel::<()>());
        });
        let _ = thread::spawn(move || {
            senderp.send_msg(unique_channel::<()>());
        });
        let ch1 = receiver.recv_msg_block();
        let ch2 = receiver.recv_msg_block();
        assert_ne!(ch1, ch2);
    });
}
