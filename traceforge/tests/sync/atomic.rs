use traceforge::{sync::atomic::*, *};
use std::sync::atomic::Ordering;
use std::sync::Arc;
// this file shows some example usage of the Must's `sync::atomic` library

#[test]
fn atomic_bool() {
    let f = || {
        let register = Arc::new(AtomicBool::new(false));
        let new_register = Arc::clone(&register);
        let t = thread::spawn(move || {
            new_register.store(true, Ordering::SeqCst);
        });
        t.join().unwrap();
        assert(register.load(Ordering::SeqCst) == true);
    };

    let stats = verify(
        Config::builder().with_keep_going_after_error(false).build(),
        f,
    );
    println!("Stats = {}, {}", stats.execs, stats.block);
}

#[test]
fn atomic_bool_cas() {
    let f = || {
        let register = Arc::new(AtomicBool::new(false));
        let new_register1 = Arc::clone(&register);
        let new_register2 = Arc::clone(&register);
        let t1 = thread::spawn(move || {
            let _ = new_register1.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst);
        });
        let t2 = thread::spawn(move || {
            let _ = new_register2.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst);
        });
        t1.join().unwrap();
        t2.join().unwrap();
        assert(register.load(Ordering::SeqCst) == true);
    };

    let stats = verify(
        Config::builder()
            .with_verbose(0)
            .with_keep_going_after_error(false)
            .build(),
        f,
    );
    println!("Stats = {}, {}", stats.execs, stats.block);
    assert_eq!(stats.execs, 2)
}

// #[test]
// fn drop_tx() {
//     let f = || {
//         let (tx, rx) = channel::<i32>();
//         drop(tx);
//         assert(rx.recv().is_err());
//     };

//     let stats = verify(
//         Config::builder().with_keep_going_after_error(false).build(),
//         f,
//     );
//     println!("Stats = {}, {}", stats.execs, stats.block);
// }

// #[test]
// fn select_from_three() {
//     let f = || {
//         // create 3 channels where
//         let (tx1, rx1) = channel::<usize>();
//         let (tx2, rx2) = channel::<usize>();
//         let (tx3, rx3) = channel::<usize>();

//         thread::spawn(move || {
//             tx1.send(1).unwrap();
//         });
//         thread::spawn(move || {
//             tx2.send(2).unwrap();
//         });
//         thread::spawn(move || {
//             tx3.send(3).unwrap();
//         });

//         let rs = vec![&rx1, &rx2, &rx3];
//         let mut v = Vec::new();

//         for _n in 0..3 {
//             let m = select_filter(&rs).0.unwrap();
//             v.push(m);
//         }

//         let vs = vec![
//             vec![1, 2, 3],
//             vec![1, 3, 2],
//             vec![2, 1, 3],
//             vec![2, 3, 1],
//             vec![3, 1, 2],
//             vec![3, 2, 1],
//         ];

//         assert!(vs.contains(&v));
//     };

//     let stats = traceforge::verify(Config::builder().build(), f);
//     println!("Stats = {}, {}", stats.execs, stats.block);
// }

// mod test {
//     use lazy_static::lazy_static;

//     use std::collections::HashMap;

//     use super::*;

//     fn channel_test_scenario(testname: &str) {
//         let (s, r) = channel::<usize>();
//         let consumer: JoinHandle<Vec<usize>> = thread::spawn(move || {
//             let mut values = Vec::new();
//             loop {
//                 let v: usize = r.recv().expect("Could not read usize");
//                 if v == 0 {
//                     return values;
//                 }
//                 values.push(v);
//             }
//         });

//         let s1 = s.clone();
//         let producer: JoinHandle<()> = thread::spawn(move || {
//             let s = s1;
//             let _ = s.send(1);
//             let _ = s.send(2);
//             let _ = s.send(3);
//             let _ = s.send(0);
//             return;
//         });

//         let producer2: JoinHandle<()> = thread::spawn(move || {
//             let _ = s.send(8);
//             return;
//         });
//         let _ = producer.join();
//         let _ = producer2.join();
//         let values: Vec<usize> = consumer.join().unwrap_or(vec![1, 2, 3, 9]);

//         println!("Values: {:?}", values);

//         assert!(vec![
//             vec![8, 1, 2, 3],
//             vec![1, 8, 2, 3],
//             vec![1, 2, 8, 3],
//             vec![1, 2, 3, 8],
//             vec![1, 2, 3],
//         ]
//         .contains(&values));
//         record(testname.to_owned(), &values);
//     }

//     lazy_static! {
//         static ref HASHMAP: Mutex<HashMap<String, HashMap<Vec<usize>, u8>>> =
//             Mutex::new(HashMap::new());
//     }

//     pub fn record(testname: String, v: &Vec<usize>) {
//         let mut testmap = HASHMAP.lock().unwrap();
//         let hmap = testmap.get_mut(&testname).unwrap();
//         let count = hmap.get(v).unwrap();
//         let new_count = *count + 1;
//         let _ = hmap.insert(v.to_vec(), new_count);
//         assert_eq!(hmap.len(), 5);
//     }

//     #[test]
//     fn verify_scenario() {
//         const TESTNAME: &str = "channel_test";
//         {
//             let mut testmap = HASHMAP.lock().unwrap();

//             let mut hs = HashMap::<Vec<usize>, u8>::new();
//             let _ = hs.insert(vec![1, 2, 3, 8], 0);
//             let _ = hs.insert(vec![1, 2, 8, 3], 0);
//             let _ = hs.insert(vec![1, 8, 2, 3], 0);
//             let _ = hs.insert(vec![8, 1, 2, 3], 0);
//             let _ = hs.insert(vec![1, 2, 3], 0);
//             let _ = testmap.insert(TESTNAME.to_owned(), hs);
//         }

//         let stats = traceforge::verify(Config::builder().build(), || {
//             channel_test_scenario(TESTNAME)
//         });
//         println!("Stats = {}, {}", stats.execs, stats.block);
//         println!("=========================");
//         let testmap = HASHMAP.lock().unwrap().clone();
//         let thistest = testmap.get(TESTNAME).unwrap();
//         for (_k, v) in thistest {
//             assert_eq!(*v, 42);
//         }
//     }

//     fn sync_channel_test_scenario(testname: &str, bound: usize) {
//         let (s, r) = sync_channel::<usize>(bound);
//         let consumer: JoinHandle<Vec<usize>> = thread::spawn(move || {
//             let mut values = Vec::new();
//             loop {
//                 let v: usize = r.recv().expect("Could not read usize");
//                 if v == 0 {
//                     return values;
//                 }
//                 values.push(v);
//             }
//         });

//         let s1 = s.clone();
//         let producer: JoinHandle<()> = thread::spawn(move || {
//             let s = s1;
//             let _ = s.send(1);
//             let _ = s.send(2);
//             let _ = s.send(3);
//             let _ = s.send(0);
//             return;
//         });

//         let producer2: JoinHandle<()> = thread::spawn(move || {
//             let _ = s.send(8);
//             return;
//         });
//         let _ = producer.join();
//         let _ = producer2.join();
//         let values: Vec<usize> = consumer.join().unwrap();

//         println!("Values: {:?}", values);

//         /*
//          * Please note that the channels have p2p semantics.
//          * In a mailbox semantics, you will not see all orders: you will only see
//          * [1,2,3], [8,1,2,3]
//          */
//         assert!(vec![
//             vec![8, 1, 2, 3],
//             vec![1, 8, 2, 3],
//             vec![1, 2, 8, 3],
//             vec![1, 2, 3, 8],
//             vec![1, 2, 3],
//         ]
//         .contains(&values));
//         record(testname.to_owned(), &values);
//     }

//     #[test]
//     fn verify_sync_scenario() {
//         const TESTNAME: &str = "sync_channel_scenario";
//         {
//             let mut testmap = HASHMAP.lock().unwrap();
//             let mut hs = HashMap::<Vec<usize>, u8>::new();
//             let _ = hs.insert(vec![1, 2, 3, 8], 0);
//             let _ = hs.insert(vec![1, 2, 8, 3], 0);
//             let _ = hs.insert(vec![1, 8, 2, 3], 0);
//             let _ = hs.insert(vec![8, 1, 2, 3], 0);
//             let _ = hs.insert(vec![1, 2, 3], 0);
//             let _ = testmap.insert(TESTNAME.to_owned(), hs);
//         }

//         let stats = traceforge::verify(Config::builder().build(), move || {
//             sync_channel_test_scenario(TESTNAME, 2)
//         });
//         println!("Stats = {}, {}", stats.execs, stats.block);
//         println!("=========================");
//         let testmap = HASHMAP.lock().unwrap().clone();
//         let hs = testmap.get(TESTNAME).unwrap();
//         for (_k, v) in hs {
//             assert_eq!(*v, 16);
//             // println!("k = {:?} v = {:?}", k, v);
//         }
//     }
// }
