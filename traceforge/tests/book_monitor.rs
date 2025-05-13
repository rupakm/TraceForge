use traceforge_macros::monitor;

use traceforge::*;

use traceforge::thread::{self, JoinHandle, ThreadId};

use traceforge::monitor_types::*;

const TEST_RUNS: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    Start(traceforge::thread::ThreadId),
    Int(u32),
}

#[monitor(Msg)]
#[derive(Clone, Debug, Default)]
pub struct MyMonitor {
    value: u32,
}

impl MyMonitor {
    pub fn new() -> Self {
        Self { value: 0 }
    }
}

impl Monitor for MyMonitor {}

impl Observer<Msg> for MyMonitor {
    fn notify(&mut self, _who: ThreadId, _whom: ThreadId, what: &Msg) -> MonitorResult {
        let old_value = self.value;
        if let Msg::Int(x) = what {
            self.value = *x;
        }
        if old_value > self.value {
            return Err("ordering error".to_string());
        }
        Ok(())
    }
}

impl Acceptor<Msg> for MyMonitor {
    fn accept(&mut self, _who: ThreadId, _whom: ThreadId, what: &Msg) -> bool {
        matches!(what, Msg::Int(_))
    }
}

#[test]
fn advanced_monitor() {
    for _ in 0..TEST_RUNS {
        let stats = crate::verify(
            crate::Config::builder()
                .with_policy(crate::SchedulePolicy::Arbitrary)
                .with_verbose(2)
                .with_seed(15528329211881264680)
                .with_cons_type(ConsType::FIFO)
                .build(),
            || {
                let _mtid1: JoinHandle<MonitorResult> =
                    start_monitor_my_monitor(MyMonitor { value: 0 });

                let tid1 = thread::spawn(|| {
                    let mtid2: Msg = crate::recv_msg_block();
                    if let Msg::Start(tid2) = mtid2 {
                        crate::send_msg(tid2, Msg::Int(1));
                        let _: Msg = crate::recv_msg_block();
                    }
                });
                let tid2 = thread::spawn(|| {
                    let mtmain: Msg = crate::recv_msg_block();
                    let mtid1: Msg = crate::recv_msg_block();
                    if let Msg::Start(tmain) = mtmain {
                        if let Msg::Start(tid1) = mtid1 {
                            crate::send_msg(tmain, Msg::Int(0));
                            let _: Msg = crate::recv_msg_block();
                            crate::send_msg(tid1, Msg::Int(2));
                        }
                    }
                });

                let tid1 = tid1.thread().id();
                let tid2 = tid2.thread().id();

                crate::send_msg(tid2, Msg::Start(thread::current().id()));
                crate::send_msg(tid2, Msg::Start(tid1));
                let _: Msg = crate::recv_msg_block();

                crate::send_msg(tid1, Msg::Start(tid2));
            },
        );
        println!("Number of execs: {} blocks: {}", stats.execs, stats.block);
        assert_eq!(stats.execs, 1);
        assert_eq!(stats.block, 0);
    }
}
