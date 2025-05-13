use std::time::{Duration, Instant};

//use traceforge::{cover, named_cover, probe, Nondet};
use traceforge::{
    cover, coverage::ExecutionObserver, monitor_types::EndCondition, thread, Config, CoverageInfo,
    ExecutionId, Nondet, SchedulePolicy, TypeNondet,
};

const FOO: &'static str = "FOO";

const HERE: &'static str = "REACHED_HERE";
const MSG_IS_42: &'static str = "MSG_IS_42";
const MSG_IS_0: &'static str = "MSG_IS_0";

#[test]
fn cover_basic_1() {
    let stats = traceforge::verify(
        Config::builder()
            .with_policy(SchedulePolicy::Arbitrary)
            .build(),
        || {
            let h = thread::spawn(move || {
                let r: i32 = traceforge::recv_msg_block();
                cover!(MSG_IS_42, r == 42);
                cover!(MSG_IS_0, r == 0)
            });
            if <bool>::nondet() {
                traceforge::send_msg(h.thread().id(), 42);
            } else {
                cover!(HERE);
                traceforge::send_msg(h.thread().id(), 11);
            }
        },
    );
    assert_eq!(stats.execs, 2);
    assert!(stats.coverage.is_covered(HERE.to_string()));
    assert_eq!(stats.coverage.covered(HERE.to_string()), 1);
    assert_eq!(stats.coverage.covered(MSG_IS_42.to_string()), 1);
    assert_eq!(stats.coverage.covered(MSG_IS_0.to_string()), 0);
}

#[test]
// This test doesn't fail, but it generates a warning because the string `FOO` and `V==0` are used in two
// source locations
fn cover_basic_2() {
    let stats = traceforge::verify(
        Config::builder()
            .with_policy(SchedulePolicy::Arbitrary)
            .with_verbose(0)
            .build(),
        || {
            let h = thread::spawn(move || {
                let _: Option<i32> = traceforge::recv_msg();
            });
            cover!(FOO);
            let r = 0_usize..4_usize;
            let v = r.nondet();
            cover!("V==0", v == 1);
            cover!(FOO);
            if v == 0 {
                cover!("V==0", v == 0); // Uh oh we reused the name
                traceforge::send_msg(h.thread().id(), 42);
            } else if v == 1 {
                cover!("V==0", v == 1); // Uh oh we reused the name
                traceforge::send_msg(h.thread().id(), 17);
            } else {
                traceforge::send_msg(h.thread().id(), 33);
            }
        },
    );
    assert_eq!(stats.execs, 8);
}

#[derive(Clone, Default)]
struct CoverageCheck {
    eids: Vec<traceforge::ExecutionId>,
}

impl CoverageCheck {
    // called at the end of model checking
    pub fn stats(&mut self) {
        assert_eq!(self.eids.len(), 1);
        print!("Msg 42 was received in the following executions");
        for eid in &self.eids {
            print!(" {eid} ");
        }
        println!();
    }
}

impl ExecutionObserver for CoverageCheck {
    // called at the end of a blocked execution
    // fn on_block(&mut self, _eid: traceforge::ExecutionId, _c: traceforge::CoverageInfo) -> () {}

    // called at the end of a normal execution
    fn after(
        &mut self,
        eid: traceforge::ExecutionId,
        _econdition: &EndCondition,
        c: traceforge::CoverageInfo,
    ) -> () {
        if c.is_covered(MSG_IS_42.to_owned()) {
            self.eids.push(eid);
        }
    }

    fn at_end_of_exploration(&mut self) -> () {
        self.stats();
    }

    fn before(&mut self, _eid: traceforge::ExecutionId) -> () {}
}

#[test]
fn cover_basic_3() {
    let covcheck = CoverageCheck::default();
    let config = Config::builder()
        .with_policy(SchedulePolicy::Arbitrary)
        .with_callback(Box::new(covcheck))
        .with_callback(Box::new(TimeMetric::new()))
        .build();
    let stats = traceforge::verify(config, || {
        let h = thread::spawn(move || {
            let r: i32 = traceforge::recv_msg_block();
            cover!(MSG_IS_42, r == 42);
        });
        if <bool>::nondet() {
            traceforge::send_msg(h.thread().id(), 42);
        } else {
            traceforge::send_msg(h.thread().id(), 11);
        }
    });
    assert_eq!(stats.execs, 2);
    assert_eq!(stats.coverage.covered(MSG_IS_42.to_string()), 1);
}

// A TimeMetric gathers timing information for the executions
// This example shows how the `ExecutionObserver`s can be used to perform different statistics gathering
struct TimeMetric {
    max: Duration,
    min: Duration,
    avg: Duration,
    num_iter: u32,

    now: Instant,
}

impl TimeMetric {
    pub fn new() -> Self {
        Self {
            max: Duration::ZERO,
            min: Duration::MAX,
            avg: Duration::ZERO,
            num_iter: 0,

            now: Instant::now(),
        }
    }
}

impl ExecutionObserver for TimeMetric {
    fn before(&mut self, _eid: traceforge::ExecutionId) -> () {
        self.now = Instant::now();
        self.num_iter += 1;
    }

    fn after(&mut self, _eid: ExecutionId, econd: &EndCondition, _c: CoverageInfo) {
        match econd {
            EndCondition::MonitorTerminated => {}
            EndCondition::AllThreadsCompleted => {
                let elapsed = self.now.elapsed();
                if self.max < elapsed {
                    self.max = elapsed;
                }
                if elapsed < self.min {
                    self.min = elapsed;
                }
                self.avg = ((self.num_iter - 1) * self.avg + elapsed) / self.num_iter;
            }
            EndCondition::Deadlock => {}
            EndCondition::FailedAssumption => {}
        }
    }

    fn at_end_of_exploration(&mut self) -> () {
        println!(
            "There were {} iterations. Max={:?}, min={:?}, avg={:?}",
            self.num_iter, self.max, self.min, self.avg
        );
    }
}
