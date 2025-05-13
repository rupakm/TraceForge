use traceforge::*;
use channel::{Builder, Channel};
use msg::Message;

fn unique_channel<T: Message + Clone + 'static>() -> Channel<T> {
    Builder::new().build()
}

#[test]
fn dropped() {
    let stats = traceforge::verify(
        Config::builder()
            .with_lossy(1)
            .with_cons_type(ConsType::FIFO)
            .build(),
        move || {
            let ch = unique_channel();
            ch.0.send_lossy_msg(1);
        },
    );
    assert_eq!(stats.execs, 2);
}

#[test]
fn lossy_channel() {
    let stats = traceforge::verify(
        Config::builder()
            .with_lossy(1)
            .with_cons_type(ConsType::FIFO)
            .build(),
        move || {
            let (sender, receiver) = unique_channel();
            sender.send_lossy_msg(1);
            sender.send_msg(2);
            let _ = thread::spawn(move || {
                assume!(receiver.recv_msg_block() == 2);
            });
        },
    );
    assert_eq!(stats.execs, 1);
}

#[test]
fn lossy_channel_rev_max() {
    for b in 1..3 {
        let stats = traceforge::verify(
            Config::builder()
                .with_lossy(b)
                .with_cons_type(ConsType::FIFO)
                .build(),
            move || {
                let (sender, receiver) = unique_channel();
                let _ = thread::spawn(move || {
                    let v = receiver.recv_msg();
                    assume!(v == Some(2));
                });
                let senderp = sender.clone();
                let _ = thread::spawn(move || {
                    sender.send_lossy_msg(1);
                });
                let _ = thread::spawn(move || {
                    senderp.send_msg(2);
                });
            },
        );
        assert_eq!(stats.execs, 2);
    }
}
