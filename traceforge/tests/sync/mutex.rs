use std::sync::Arc;

use traceforge::sync::mutex::Mutex;
use traceforge::thread;
// this file shows some example usage of the Must's `sync::mutex` library

#[test]
fn single_producer() {
    let f = || {
        let m = Arc::new(Mutex::new(5));
        let mclone = m.clone();
        traceforge::future::block_on(async move {
            let mut v = mclone.lock().await;
            traceforge::assert(*v == 5);
            *v = *v + 1;
        });
        traceforge::future::block_on(async {
            let v = m.lock().await;
            traceforge::assert(*v == 6);
        });
    };

    let stats = traceforge::verify(
        traceforge::Config::builder()
            .with_keep_going_after_error(false)
            .build(),
        f,
    );
    println!("Stats = {}, {}", stats.execs, stats.block);
}

#[test]
fn multiple_producer() {
    let f = || {
        let m = Arc::new(Mutex::new(5));
        let m1 = m.clone();
        let m2 = m.clone();
        let t1 = thread::spawn(move || {
            traceforge::future::block_on(async {
                let mut v = m1.lock().await;
                *v = *v + 1;
            });
        });
        let t2 = thread::spawn(move || {
            traceforge::future::block_on(async {
                let mut v = m2.lock().await;
                *v = *v + 1;
            });
        });
        let _ = t1.join();
        let _ = t2.join();
        assert!(*m.blocking_lock() == 7)
    };

    let stats = traceforge::verify(
        traceforge::Config::builder()
            .with_verbose(2)
            .with_keep_going_after_error(false)
            .build(),
        f,
    );
    assert_eq!(stats.execs, 2); // two executions: T1 releases lock before T2 and vice versa
                                //
                                // for each of these, the lock in the main thread does not add any
                                // more executions
    assert_eq!(stats.block, 0);
    println!("Stats = {}, {}", stats.execs, stats.block);
}

#[test]
fn race_condition() {
    let stats = traceforge::verify(traceforge::Config::default(), || {
        let mutex = Arc::new(Mutex::new(1));

        let mutex2 = mutex.clone();
        let f1 = traceforge::future::spawn(async move {
            let mut val = mutex2.lock().await;
            *val += 1;
        });

        let mutex2 = mutex.clone();
        let f2 = traceforge::future::spawn(async move {
            let mut val = mutex2.lock().await;
            *val *= 10;
        });

        traceforge::future::block_on(f1).unwrap();
        traceforge::future::block_on(f2).unwrap();

        let ending_val = traceforge::future::block_on(async move {
            let val = mutex.lock().await;
            *val
        });

        traceforge::cover!("20", ending_val == 20); // (1 + 1) * 10
        traceforge::cover!("11", ending_val == 11); // 1 + (1 * 10)
    });
    assert_eq!((2, 0), (stats.execs, stats.block));
    assert!(stats.coverage.is_covered("20".into()));
    assert!(stats.coverage.is_covered("11".into()));
}

#[test]
fn mutex_doc_test() {
    let stats = traceforge::verify(traceforge::Config::default(), || {
        traceforge::future::block_on(async {
            let count = Arc::new(Mutex::new(0));

            for i in 0..3 {
                let my_count = Arc::clone(&count);
                traceforge::thread::spawn(move || {
                    for j in 0..2 {
                        let mut lock = my_count.blocking_lock();
                        *lock += 1;
                        println!("{} {} {}", i, j, lock);
                    }
                });
            }

            let endval = *count.lock().await;
            traceforge::cover!("0", endval == 0);
            traceforge::cover!("1", endval == 1);
            traceforge::cover!("2", endval == 2);
            traceforge::cover!("3", endval == 3);
            traceforge::cover!("4", endval == 4);
            traceforge::cover!("5", endval == 5);
            traceforge::cover!("6", endval == 6);
            traceforge::cover!("7", endval == 7);

            println!("Count hit {endval}.");
        });
    });
    print!("Stats = {}, {}", stats.execs, stats.block);
    assert!(stats.coverage.is_covered("0".into()));
    assert!(stats.coverage.is_covered("1".into()));
    assert!(stats.coverage.is_covered("2".into()));
    assert!(stats.coverage.is_covered("3".into()));
    assert!(stats.coverage.is_covered("4".into()));
    assert!(stats.coverage.is_covered("5".into()));
    assert!(stats.coverage.is_covered("6".into()));

    assert!(!stats.coverage.is_covered("7".into()));
}
