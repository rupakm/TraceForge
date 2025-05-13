use traceforge::{recv_tagged_msg, recv_tagged_msg_block, send_msg, thread::spawn, verify, Config};

#[test]
#[should_panic(expected = "wrong message return type; expecting bool but got u32")]
fn test_recv_wrong_type_nonblocking() {
    verify(Config::builder().build(), || {
        let tid = spawn(|| {
            let _: Option<bool> = recv_tagged_msg(|_, _| true);
        })
        .thread()
        .id();
        send_msg(tid, 0u32);
    });
}

#[test]
#[should_panic(expected = "wrong message return type; expecting bool but got u32")]
fn test_recv_wrong_type() {
    verify(Config::builder().build(), || {
        let tid = spawn(|| {
            let _: bool = recv_tagged_msg_block(|_, _| true);
        })
        .thread()
        .id();
        send_msg(tid, 0u32);
    });
}
