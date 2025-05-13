use paste::paste;

use crate::thread::ThreadId;

use crate::monitor_types;

macro_rules! mk_monitor {
    ($t:tt) => {
        paste! {  [< start_monitor_ $t >]($t) }
    };
    ($t:tt, $($e:expr),* $(,)?) => {
        paste! { [< start_monitor_ $t >](<$t>::new($($e),* )) }
    };
    }

#[cfg(test)]
mod test {

    use super::*;
    use crate::msg::Message;
    use crate::thread::{self, JoinHandle, ThreadId};
    use crate::Config;
    use crate::ConsType;
    use std::collections::BTreeMap;

    const TEST_RUNS: u32 = 1000;

    #[monitor(M1, M2, Foo)]
    #[derive(Clone, Debug)]
    pub struct MyMonitor {
        value: u32,
    }

    #[monitor(M1)]
    #[derive(Clone, Debug)]
    pub struct MyOtherMonitor;

    #[derive(Debug, Clone, PartialEq)]
    pub struct M1;

    #[derive(Debug, Clone, PartialEq)]
    pub struct M2;

    #[derive(Debug, Clone, PartialEq)]
    pub struct Foo;

    #[derive(Debug, Clone, PartialEq)]
    pub struct M4;

    impl MyMonitor {
        pub fn new(_f: u8) -> Self {
            // _g: u16) -> Self {
            Self { value: 0 }
        }
    }

    impl Monitor for MyMonitor {}

    impl Observer<M1> for MyMonitor {
        fn notify(&mut self, who: ThreadId, whom: ThreadId, what: &M1) -> MonitorResult {
            //println!("Monitor: {:?} == {:?} => {:?}", who, what, whom);
            Ok(())
        }
    }

    impl Observer<M2> for MyMonitor {}

    impl Observer<Foo> for MyMonitor {}

    impl Monitor for MyOtherMonitor {}

    impl Observer<M1> for MyOtherMonitor {}

    /*
    pub fn monitor_code() {
        let mut mon = MyMonitor;
        let (who, whom, m): (ThreadId, ThreadId, MyMonitorMsg) = crate::recv_msg_block();
        println!("Who = {:?}, Whom = {:?}, what = {:?}", who, whom, &m);
        let _ = (&mut mon as &mut dyn Observer<MyMonitorMsg>).notify(who, whom, &m);
    }

    pub fn send_monitor(t: ThreadId, m: Box<dyn Message>) {
        let msg = m.clone();
        if let Ok(msg) = msg.as_any().downcast::<(ThreadId, ThreadId, M1)>() {
            println!("M1");
            let msg = *msg;
            let tid1 = msg.0;
            let tid2 = msg.1;
            let msg = msg.2;
            let m: MyMonitorMsg = <M1 as Into<MyMonitorMsg>>::into(msg);
            println!("HERE M1");
            crate::send_msg(t, (tid1, tid2, m));

            return;
        }
        let msg = m.clone();
        if let Ok(msg) = msg.as_any().downcast::<M2>() {
            println!("M2");
            let m: MyMonitorMsg = <M2 as Into<MyMonitorMsg>>::into(*msg);
            crate::send_msg(t, m);
            return;
        }
        panic!("Unknown message type");
    }

    pub fn send_monitor2(t: ThreadId, msg: Box<dyn Message>) {
        let msg1 = msg.clone();
        if let Ok(msg) = msg1.as_any().downcast::<(ThreadId, ThreadId, M1)>() {
            println!("HERE");

            let msg = *msg;
            let tid1 = msg.0;
            let tid2 = msg.1;
            let msg = msg.2;
            let m: MyMonitorMsg = <M1 as Into<MyMonitorMsg>>::into(msg);
            println!("HERE2");
            crate::send_msg(t, (tid1, tid2, m));
            return;
        }
        let msg1 = msg.clone();
        if let Ok(msg) = msg1.as_any().downcast::<(ThreadId, ThreadId, M2)>() {
            let msg = *msg;
            let tid1 = msg.0;
            let tid2 = msg.1;
            let msg = msg.2;
            let m: MyMonitorMsg = <M2 as Into<MyMonitorMsg>>::into(msg);
            crate::send_msg(t, (tid1, tid2, m));
            return;
        }
        panic!("Unknown message type");
    }
    */

    #[test]
    fn run() {
        for _ in 0..TEST_RUNS {
            let mut creators = BTreeMap::new();
            creators.insert(
                thread::ThreadId { task_id: 3.into() },
                create_msg_for_monitor_MyMonitor
                    as fn(Val) -> Option<Val>,
            );
            creators.insert(
                thread::ThreadId { task_id: 4.into() },
                create_msg_for_monitor_MyOtherMonitor
                    as fn(Val) -> Option<Val>,
            );
            let stats = crate::verify(
                Config::builder()
                    .with_policy(crate::SchedulePolicy::Arbitrary)
                    .with_cons_type(ConsType::WB)
                    .with_monitor_info(crate::MonitorInfo {
                        is_present: true,
                        create_msgs: creators,
                    })
                    .build(),
                || {
                    let tid1 = thread::spawn(|| {
                        crate::send_msg(thread::ThreadId { task_id: 2.into() }, M1);
                        let _: Foo = crate::recv_msg_block();
                        crate::send_msg(thread::ThreadId { task_id: 2.into() }, Foo);
                        crate::send_msg(thread::ThreadId { task_id: 2.into() }, M4);
                    });
                    let tid2 = thread::spawn(|| {
                        crate::send_msg(thread::ThreadId { task_id: 1.into() }, Foo);
                    });
                    let mtid1: JoinHandle<MonitorResult> = mk_monitor!(MyMonitor, 0); //start_monitor_MyMonitor(MyMonitor {value : 0}); // mk_monitor!(MyMonitor);
                    let mtid2: JoinHandle<MonitorResult> = mk_monitor!(MyOtherMonitor); // start_monitor_MyOtherMonitor(MyOtherMonitor);

                    let tid1 = tid1.thread().id();
                    let tid2 = tid2.thread().id();

                    //send_monitor_MyMonitor(mtid1.thread().id(),Box::new((Some(tid1), Some(tid2), m.clone())));
                    //send_monitor_MyOtherMonitor(mtid2.thread().id(),Box::new((Some(tid1), Some(tid2), m.clone())));
                    //println!("return value {:?}",create_msg_for_monitor_MyMonitor(Box::new(m.clone())));
                    //create_msg_for_monitor_MyMonitor(Box::new(mp.clone()));
                    //println!("return value {:?}",create_msg_for_monitor_MyMonitor(Box::new(mpp.clone())));
                    terminate_monitor_MyMonitor(mtid1.thread().id());
                    terminate_monitor_MyOtherMonitor(mtid2.thread().id());

                    /*
                    // This is what the subscription list will look like
                    let q: Vec<(
                        ThreadId,
                        fn(ThreadId, Box<dyn Message>) -> (),
                        fn(ThreadId) -> (),
                    )> = vec![
                        (
                            mtid1.thread().id(),
                            send_monitor_MyMonitor,
                            terminate_monitor_MyMonitor,
                        ),
                        (
                            mtid2.thread().id(),
                            send_monitor_MyOtherMonitor,
                            terminate_monitor_MyOtherMonitor,
                        ),
                    ];
                    for (tid, f, _) in &q {
                        f(*tid, Box::new((Some(tid1), Some(tid2), m.clone())));
                    }

                    // terminate_monitor_MyMonitor(mtid1.thread().id());

                    for (tid, _, kill) in &q {
                        // send terminate
                        kill(*tid);
                    }
                    */

                    let ok1: MonitorResult = mtid1.join().unwrap();
                    let ok2: MonitorResult = mtid2.join().unwrap();
                    assert!(ok1.is_ok() && ok2.is_ok());
                },
            );
            println!("Total number of executions {}", stats.execs);
            assert_eq!(stats.execs, 18);
        }
    }
}
