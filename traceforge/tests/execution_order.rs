use std::sync::Arc;

use traceforge::{nondet, thread::current_id, Config};
use futures::lock::Mutex;

#[test]
fn test_out_of_order_blocking_recv() {
    traceforge::verify(Config::builder().build(), || {
        let sent: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let sent_clone = sent.clone();

        let main_tid = current_id();
        println!("Main thread prepares to spawn");
        let _ = traceforge::thread::spawn(move || {
            println!("Spawned prepares to send");
            *sent_clone.try_lock().unwrap() = true;
            traceforge::send_msg(main_tid, 1);
            println!("Spawned thread has sent");
        })
        .thread()
        .id();
        println!("Main thread prepares to recv");
        let _: i32 = traceforge::recv_msg_block();
        if !*sent.try_lock().unwrap() {
            panic!("Received a message that was not yet sent.");
        }
        println!("Main thread has received");

        let _ = nondet(); // Nondet causes a revisit.
    });
}

#[test]
fn test_out_of_order_blocking_recv2() {
    traceforge::verify(Config::builder().build(), || {
        let sent: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let sent_clone = sent.clone();

        let main_tid = current_id();
        println!("Main thread prepares to spawn");
        let _ = traceforge::thread::spawn(move || {
            println!("Spawned prepares to send");
            *sent_clone.try_lock().unwrap() = true;
            traceforge::send_msg(main_tid, 1);
            println!("Spawned thread has sent");
        })
        .thread()
        .id();
        let _ = nondet(); // Nondet causes a revisit.
        println!("Main thread prepares to recv");
        let _: i32 = traceforge::recv_msg_block();
        if !*sent.try_lock().unwrap() {
            panic!("Received a message that was not yet sent.");
        }
        println!("Main thread has received");
    });
}

#[test]
fn test_out_of_order_nonblocking_recv() {
    traceforge::verify(Config::builder().build(), || {
        let sent: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let sent_clone = sent.clone();

        let main_tid = current_id();
        println!("Main thread prepares to spawn");
        let _ = traceforge::thread::spawn(move || {
            println!("Spawned prepares to send");
            *sent_clone.try_lock().unwrap() = true;
            traceforge::send_msg(main_tid, 1);
            println!("Spawned thread has sent");
        })
        .thread()
        .id();
        println!("Main thread prepares to recv");
        let received: Option<i32> = traceforge::recv_msg();
        if received.is_some() && !*sent.try_lock().unwrap() {
            panic!("Received a message that was not yet sent.");
        }
        println!("Main thread has received");
        let _ = nondet(); // Nondet causes a revisit.
    });
}

#[test]
fn test_out_of_order_nonblocking_recv2() {
    traceforge::verify(Config::builder().build(), || {
        let sent: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let sent_clone = sent.clone();

        let main_tid = current_id();
        println!("Main thread prepares to spawn");
        let _ = traceforge::thread::spawn(move || {
            println!("Spawned prepares to send");
            *sent_clone.try_lock().unwrap() = true;
            traceforge::send_msg(main_tid, 1);
            println!("Spawned thread has sent");
        })
        .thread()
        .id();
        let _ = nondet(); // Nondet causes a revisit.
        println!("Main thread prepares to recv");
        let received: Option<i32> = traceforge::recv_msg();
        if received.is_some() && !*sent.try_lock().unwrap() {
            panic!("Received a message that was not yet sent.");
        }
        println!("Main thread has received");
    });
}

/// This test shows the same problem as the previous one, but with thread::join() taking
/// the place of recv_msg_block().
#[test]
fn test_out_of_order_join() {
    traceforge::verify(Config::builder().build(), || {
        let sent: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let sent_clone = sent.clone();

        println!("Main thread prepares to spawn");
        let jh = traceforge::thread::spawn(move || {
            println!("Spawned process returning a value");
            *sent_clone.try_lock().unwrap() = true;
            1
        });
        println!("Main thread prepares to join");
        let v: i32 = jh.join().unwrap();
        if !*sent.try_lock().unwrap() {
            panic!("Received a join value before the joined thread was executed.");
        }
        println!("Main thread has received {} ", v);
        let _ = nondet(); // Nondet causes a revisit.
    });
}

/// This test shows the same problem as the previous one, but with thread::join() taking
/// the place of recv_msg_block().
#[test]
fn test_out_of_order_future() {
    traceforge::verify(
        Config::builder()
            .with_policy(traceforge::SchedulePolicy::Arbitrary)
            .build(),
        || {
            traceforge::future::block_on(async {
                let sent: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
                let sent_clone = sent.clone();

                println!("Main thread prepares to spawn");
                let fut = traceforge::future::spawn(async move {
                    println!("Spawned process returning a value");
                    *sent_clone.try_lock().unwrap() = true;
                    1
                });
                println!("Main thread prepares to join");
                let v: i32 = fut.await.unwrap();
                if !*sent.try_lock().unwrap() {
                    panic!("Received a join value before the joined thread was executed.");
                }
                println!("Main thread has received {} ", v);
                let _ = nondet(); // Nondet causes a revisit.
            });
        },
    );
}

#[test]
fn two_receives_two_sends() {
    let stats = traceforge::verify(Config::default(), || {
        let tid1 = traceforge::thread::spawn(|| {
            let _: String = traceforge::recv_tagged_msg_block(|_, tag| tag == Some(1));
            let _: String = traceforge::recv_tagged_msg_block(|_, tag| tag == Some(2));
        })
        .thread()
        .id();

        traceforge::thread::spawn(move || {
            traceforge::send_tagged_msg(tid1, 2, "value2".to_string());
            traceforge::send_tagged_msg(tid1, 1, "value1".to_string());
        });
    });
    println!("Executions: {:?}", stats);
}
