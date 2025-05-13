use std::any::Any;

use traceforge_macros::monitor;

use traceforge::*;

use traceforge::thread::{self, current_id, main_thread_id, spawn, JoinHandle, ThreadId};

use traceforge::monitor_types::*;

use serial_test::serial;
use utils::assert_panic_contains;

mod utils;

const TEST_RUNS: u32 = 1000;

#[monitor(M1, M2, Foo)]
#[derive(Clone, Debug, Default)]
pub struct MyMonitor {}

#[monitor(M1)]
#[derive(Clone, Debug, Default)]
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
    pub fn new() -> Self {
        Self {}
    }
}

impl Monitor for MyMonitor {}

impl Observer<M1> for MyMonitor {
    fn notify(&mut self, _who: ThreadId, _whom: ThreadId, _what: &M1) -> MonitorResult {
        // Err("something".to_string())
        Ok(())
    }
}

impl Acceptor<M1> for MyMonitor {
    fn accept(&mut self, _who: ThreadId, _whom: ThreadId, _what: &M1) -> bool {
        true
    }
}

impl Acceptor<M2> for MyMonitor {
    fn accept(&mut self, _who: ThreadId, _whom: ThreadId, _what: &M2) -> bool {
        true
    }
}

impl Acceptor<Foo> for MyMonitor {
    fn accept(&mut self, _who: ThreadId, _whom: ThreadId, _what: &Foo) -> bool {
        false
    }
}

impl Acceptor<M1> for MyOtherMonitor {
    fn accept(&mut self, _who: ThreadId, _whom: ThreadId, _what: &M1) -> bool {
        true
    }
}

impl Observer<M2> for MyMonitor {}

impl Observer<Foo> for MyMonitor {}

impl Monitor for MyOtherMonitor {}

impl Observer<M1> for MyOtherMonitor {}

#[test]
#[serial]
fn run() {
    for _ in 0..TEST_RUNS {
        let stats = crate::verify(
            crate::Config::builder()
                .with_policy(crate::SchedulePolicy::Arbitrary)
                .with_verbose(0)
                .with_seed(15528329211881264680)
                .with_cons_type(ConsType::FIFO)
                .build(),
            || {
                let mtid1: JoinHandle<MonitorResult> = start_monitor_my_monitor(MyMonitor {});
                let mtid2: JoinHandle<MonitorResult> =
                    start_monitor_my_other_monitor(MyOtherMonitor);

                let tid1 = thread::spawn(|| {
                    let tid2: ThreadId = crate::recv_msg_block();

                    crate::send_msg(tid2, M1);
                    let _: Foo = crate::recv_msg_block();
                    crate::send_msg(tid2, Foo);
                    crate::send_msg(tid2, M4);
                });
                let tid2 = thread::spawn(|| {
                    let tmain: ThreadId = crate::recv_msg_block();
                    let tid1: ThreadId = crate::recv_msg_block();
                    crate::send_msg(tmain, tmain);

                    let _: M1 = crate::recv_msg_block();
                    crate::send_msg(tid1, Foo);
                });

                let tid1 = tid1.thread().id();
                let tid2 = tid2.thread().id();

                crate::send_msg(tid2, thread::current().id());
                crate::send_msg(tid2, tid1);
                let _: ThreadId = crate::recv_msg_block();

                crate::send_msg(tid1, tid2);

                // Given the way monitors are defined right now, as long as the monitor doesn't do
                // anything in on_stop, then terminating a monitor does nothing, since any
                // counterexample is found as soon as the monitor received the message that violates
                // the specification, which means that no other thread will run any longer, so no
                // other thread will be able to terminate it or join it either.

                terminate_monitor_my_monitor(mtid1.thread().id());
                terminate_monitor_my_other_monitor(mtid2.thread().id());

                let ok1: MonitorResult = mtid1.join().unwrap();
                let ok2: MonitorResult = mtid2.join().unwrap();
                assert!(ok1.is_ok() && ok2.is_ok());
            },
        );
        assert_eq!((stats.execs, stats.block), (4, 0));
    }
}

/// Define a monitor that receives integers and ensures that they are strictly increasing
#[monitor(i32)]
#[derive(Clone, Debug, Default)]
pub struct IsIncreasing {
    value: i32,
}

impl Observer<i32> for IsIncreasing {
    fn notify(&mut self, _who: ThreadId, _whom: ThreadId, v: &i32) -> MonitorResult {
        if *v >= self.value {
            self.value = v + 1;
            Ok(())
        } else {
            Err(format!(
                "IsIncreasing expected >= {} but got {}",
                self.value, v
            ))
        }
    }
}

impl Acceptor<i32> for IsIncreasing {}

impl Monitor for IsIncreasing {}

#[test]
#[serial]
fn test_instantiate_monitor() {
    let stats = verify(Config::builder().build(), || {
        let _mon_handle = start_monitor_is_increasing(IsIncreasing { value: 0 });
    });
    assert_eq!(stats.execs, 1);
    assert_eq!(stats.block, 0);
}

#[test]
#[serial]
fn test_terminate_monitor() {
    let stats = verify(Config::builder().build(), || {
        let mon_handle = start_monitor_is_increasing(IsIncreasing { value: 0 });
        terminate_monitor_is_increasing(mon_handle.thread().id());
    });
    assert_eq!(stats.execs, 1);
    assert_eq!(stats.block, 0);
}

#[test]
#[serial]
fn test_join_without_terminate() {
    let stats = verify(Config::builder().build(), || {
        let mon_handle = start_monitor_is_increasing(IsIncreasing { value: 0 });
        let res = mon_handle.join();
        assert!(res.unwrap().is_ok()); // Unwrap joined value, make sure monitor return == Ok
    });
    assert_eq!(stats.execs, 0);
    assert_eq!(stats.block, 1); // Blocks because you have to terminate the monitor
}

#[test]
#[serial]
fn test_terminate_then_join() {
    let stats = verify(Config::builder().build(), || {
        let mon_handle = start_monitor_is_increasing(IsIncreasing { value: 0 });
        terminate_monitor_is_increasing(mon_handle.thread().id());
        let res = mon_handle.join();
        assert!(res.unwrap().is_ok()); // Unwrap joined value, make sure monitor return == Ok
    });
    assert_eq!(stats.execs, 1);
    assert_eq!(stats.block, 0);
}

#[test]
#[serial]
#[should_panic(expected = "assertion failed")]
fn test_find_counterexample() {
    let _stats = verify(Config::builder().build(), || {
        start_monitor_is_increasing(IsIncreasing { value: 0 });
        let child_handle = spawn(|| {
            let _: i32 = recv_msg_block();
            let _: i32 = recv_msg_block();
        });
        send_msg(child_handle.thread().id(), 5); // Ok
        send_msg(child_handle.thread().id(), 5); // Fails--it did not increase from 5.
                                                 // Notice that the monitor is not shut down and there is no code to assert that it is
                                                 // returning an ok value.
    });
    unreachable!();
}

#[test]
#[serial]
fn test_no_counterexample_if_shutdown_early() {
    // Shows a slightly weird case where the monitor would reject the input, but when
    // shut down early, it will not observe the violating messages.
    let stats = verify(Config::builder().build(), || {
        let mon_handle = start_monitor_is_increasing(IsIncreasing { value: 0 });
        let child_handle = spawn(|| {
            let _: i32 = recv_msg_block();
            let _: i32 = recv_msg_block();
        });
        terminate_monitor_is_increasing(mon_handle.thread().id());
        let res = mon_handle.join();
        assert!(res.is_ok());

        send_msg(child_handle.thread().id(), 5); // Ok
        send_msg(child_handle.thread().id(), 5); // Still ok because monitor is stopped
    });
    assert_eq!(stats.execs, 1);
    assert_eq!(stats.block, 0);
}

#[should_panic(expected = "Monitors must be spawned before")]
#[test]
#[serial]
fn test_no_counterexample_if_started_late() {
    let stats = verify(Config::builder().build(), || {
        let child_handle = spawn(|| {
            let _: i32 = recv_msg_block();
            let _: i32 = recv_msg_block();
        });
        send_msg(child_handle.thread().id(), 5); // Ok
        send_msg(child_handle.thread().id(), 5); // Still ok because monitor is not started

        let mon_handle = start_monitor_is_increasing(IsIncreasing { value: 0 });
        terminate_monitor_is_increasing(mon_handle.thread().id());
        let res = mon_handle.join();
        assert!(res.unwrap().is_ok()); // Unwrap joined value, then make sure monitor return == Ok
    });
    assert_eq!(stats.execs, 1);
    assert_eq!(stats.block, 0);
}

/// Define a monitor whose on_stop checks that at least 2 messages were received.
#[monitor(i32)]
#[derive(Clone, Debug, Default)]
pub struct GotTwo {
    messages_seen: u32,
}

impl Observer<i32> for GotTwo {
    fn notify(&mut self, _who: ThreadId, _whom: ThreadId, _v: &i32) -> MonitorResult {
        self.messages_seen += 1;
        Ok(())
    }
}

impl Acceptor<i32> for GotTwo {}

impl Monitor for GotTwo {
    fn on_stop(&mut self, _: &ExecutionEnd) -> MonitorResult {
        if self.messages_seen >= 2 {
            Ok(())
        } else {
            Err(format!("Only got {} messages", self.messages_seen))
        }
    }
}

#[test]
#[serial]
fn test_got_two_no_counterexample() {
    let stats = verify(Config::builder().build(), || {
        let mon_handle = start_monitor_got_two(GotTwo { messages_seen: 0 });

        let child_handle = spawn(|| {
            let _: i32 = recv_msg_block();
            let _: i32 = recv_msg_block();
        });
        send_msg(child_handle.thread().id(), 5);
        send_msg(child_handle.thread().id(), 5);

        terminate_monitor_got_two(mon_handle.thread().id());
        let res = mon_handle.join();
        assert!(res.unwrap().is_ok()); // Unwrap joined value, then make sure monitor return == Ok
    });
    assert_eq!(stats.execs, 1);
    assert_eq!(stats.block, 0);
}

#[test]
#[serial]
#[should_panic(expected = "assertion failed")]
fn test_got_two_with_counterexample() {
    let _stats = verify(Config::builder().build(), || {
        let mon_handle = start_monitor_got_two(GotTwo { messages_seen: 0 });

        let child_handle = spawn(|| {
            let _: i32 = recv_msg_block();
        });
        send_msg(child_handle.thread().id(), 5);

        terminate_monitor_got_two(mon_handle.thread().id());
        let res = mon_handle.join();
        assert!(res.unwrap().is_ok()); // Unwrap joined value, then make sure monitor return == Ok
    });
    unreachable!();
}

/// Define a monitor whose behavior is controlled by an enum; this helps with being able to
/// have many test cases without having to copy-paste a ton of code and come up with a bunch
/// of unique monitor names.
#[derive(Clone, Debug, Default, Copy)]
pub enum Action {
    /// The monitor returns Ok(()) to signify that the specification is met.
    #[default]
    MonitorReturnsOk,
    /// The monitor returns Err(_) to signify that the specification is not met.
    MonitorReturnsErr,
    /// The monitor panics, often as a result of an false assert!(). The spec is not met.
    MonitorPanics,
    /// The monitor tries to spawn a thread, or use some other Must API such as sending messages
    /// etc. This is an illegal use of a monitor, and it should panic with an appropriate message.
    MonitorTriesToSpawn,
}

#[derive(Clone, Debug, PartialEq)]
struct PublishedThreadId {
    thread_id: ThreadId,
}

#[derive(Clone, Debug, PartialEq)]
struct PublishedByMonitor {}

#[monitor(ThreadId)]
#[derive(Clone, Debug, Default)]
pub struct ActionMonitor {
    action: Action,
    last_msg: Option<ThreadId>,
}

impl ActionMonitor {
    pub fn new(action: Action) -> Self {
        Self {
            action,
            last_msg: None,
        }
    }
}

impl Acceptor<ThreadId> for ActionMonitor {}

impl Observer<ThreadId> for ActionMonitor {
    fn notify(&mut self, _who: ThreadId, _whom: ThreadId, what: &ThreadId) -> MonitorResult {
        // Publish something here just to exercise the code path, showing that is is legal to
        // publish from the notify function.
        publish(PublishedByMonitor {});
        self.last_msg = Some(what.clone());
        Ok(())
    }
}

impl Monitor for ActionMonitor {
    fn on_stop(&mut self, execution_end: &ExecutionEnd) -> MonitorResult {
        if self.last_msg.is_none() {
            panic!("Didn't receive the message");
        }
        let published = execution_end.get_published::<PublishedThreadId>();
        assert!(!published.is_empty());
        match self.action {
            Action::MonitorReturnsOk => Ok(()),
            Action::MonitorReturnsErr => Err("Expected Monitor Err".to_owned()),
            Action::MonitorPanics => panic!("Expected Monitor Panic"),
            Action::MonitorTriesToSpawn => {
                log::info!("Monitor tries to spawn a thread");
                spawn(|| {});
                Ok(())
            }
        }
    }
}

const DOT_FILE: &str = "/tmp/monitor.rs.dot";
const ERROR_FILE: &str = "/tmp/monitor.rs.error";
const TRACE_FILE: &str = "/tmp/monitor.rs.trace.json";

fn config() -> Config {
    // Note: counterexamples should not print the dot file or trace file
    // because those are printed at the end of executions.
    // But we'll still set the parameters anyway to see if this trips
    // any panics.
    Config::builder()
        .with_error_trace(ERROR_FILE)
        .with_dot_out(DOT_FILE)
        .with_trace_out(TRACE_FILE)
        .build()
}

fn remove_old_files() {
    let _ = std::fs::remove_file(ERROR_FILE);
    let _ = std::fs::remove_file(DOT_FILE);
    let _ = std::fs::remove_file(TRACE_FILE);
}

fn verify_or_estimate<F>(ver: bool, f: F)
where
    F: Fn() + Send + Sync + 'static,
{
    if ver {
        traceforge::verify(config(), f);
    } else {
        traceforge::estimate_execs_with_config(config(), f, 1000);
    }
}

/// There are several different ways that a monitor's on_stop can be invoked.
/// 1. It can be terminated explicitly before the threads stop (this function)
/// 2. It can run all the threads to completion
/// 3. It can deadlock with one or more threads blocking on recv_msg_block or join
fn execution_terminates_monitor(ver: bool, action: Action) {
    remove_old_files();
    verify_or_estimate(ver, move || {
        let expected_result = match action {
            Action::MonitorReturnsErr => Err("Expected Monitor Err".to_owned()),
            _ => Ok(()),
        };
        let mon_handle = start_monitor_action_monitor(ActionMonitor::new(action.clone()));
        publish(PublishedThreadId {
            thread_id: current_id(),
        });
        send_msg(current_id(), current_id()); // Allow monitor to observe message
        terminate_monitor_action_monitor(mon_handle.thread().id());
        let monitor_result = mon_handle.join().unwrap();
        assert_eq!(monitor_result, expected_result);
    });
}

/// There are several different ways that a monitor's on_stop can be invoked.
/// 1. It can be terminated explicitly before the threads stop
/// 2. It can run all the threads to completion (this function)
/// 3. It can deadlock with one or more threads blocking on recv_msg_block or join
fn execution_finishes_threads(ver: bool, action: Action) {
    remove_old_files();
    // Note: because a monitor is also a daemon thread, this case is actually equivalent to a
    // blocked execution since any monitor which is not explicitly terminated will be
    // a blocked (daemon) thread.
    verify_or_estimate(ver, move || {
        let _ = start_monitor_action_monitor(ActionMonitor::new(action.clone()));
        publish(PublishedThreadId {
            thread_id: current_id(),
        });
        send_msg(current_id(), current_id()); // Allow monitor to observe message
    });
}

/// There are several different ways that a monitor's on_stop can be invoked.
/// 1. It can be terminated explicitly before the threads stop
/// 2. It can run all the threads to completion
/// 3. It can deadlock with one or more threads blocking on recv_msg_block or join (this function)
fn execution_deadlocks(ver: bool, action: Action) {
    remove_old_files();
    verify_or_estimate(ver, move || {
        let _ = start_monitor_action_monitor(ActionMonitor::new(action.clone()));
        publish(PublishedThreadId {
            thread_id: current_id(),
        });
        send_msg(current_id(), current_id()); // Allow monitor to observe message
        let _: ThreadId = recv_msg_block();
        let _never_arrives: u32 = recv_msg_block(); // Never completes
    });
}

// Then we have 12 total tests, with 4 monitor actions and 3 ways to finish an execution
// (other than terminating it prematurely using assert or assume.)
// We also double this because we need to test behavior when calling `verify` and when
// calling `estimating_execs`

#[test]
#[serial]
fn verify_terminates_monitor_and_monitor_is_ok() {
    execution_terminates_monitor(true, Action::MonitorReturnsOk);
}
#[test]
#[serial]
fn estimate_terminates_monitor_and_monitor_is_ok() {
    execution_terminates_monitor(false, Action::MonitorReturnsOk);
}

fn execution_terminates_monitor_and_monitor_returns_err(ver: bool) {
    let action = Action::MonitorReturnsErr;
    let result = std::panic::catch_unwind(|| {
        execution_terminates_monitor(ver, action);
    });
    // This doesn't generate a counterexample because the body of the Must program
    // terminates the monitor, and is therefore responsible for checking the return value
    // of the monitor.
    assert!(
        result.is_ok(),
        "No error expected if the monitor is terminated by the main thread"
    );
}

#[test]
#[serial]
fn verify_terminates_monitor_and_monitor_returns_err() {
    execution_terminates_monitor_and_monitor_returns_err(true);
}

#[test]
#[serial]
fn estimate_terminates_monitor_and_monitor_returns_err() {
    execution_terminates_monitor_and_monitor_returns_err(false);
}

fn execution_terminates_monitor_and_monitor_panics(ver: bool) {
    let action = Action::MonitorPanics;
    let result = std::panic::catch_unwind(|| {
        execution_terminates_monitor(ver, action);
    });
    assert_counterexample(action, result, "Expected Monitor Panic");
}

#[test]
#[serial]
fn verify_terminates_monitor_and_monitor_panics() {
    execution_terminates_monitor_and_monitor_panics(true);
}

#[test]
#[serial]
fn estimate_terminates_monitor_and_monitor_panics() {
    execution_terminates_monitor_and_monitor_panics(false);
}

fn execution_finishes_threads_and_monitor_is_ok(ver: bool) {
    let action = Action::MonitorReturnsOk;
    let result = std::panic::catch_unwind(|| {
        execution_finishes_threads(ver, action);
    });
    assert_counterexample(action, result, "Ok");
}

#[test]
#[serial]
fn verify_finishes_threads_and_monitor_is_ok() {
    execution_finishes_threads_and_monitor_is_ok(true);
}

#[test]
#[serial]
fn estimate_finishes_threads_and_monitor_is_ok() {
    execution_finishes_threads_and_monitor_is_ok(false);
}

fn execution_finishes_threads_and_monitor_returns_err(ver: bool) {
    let action = Action::MonitorReturnsErr;
    let result = std::panic::catch_unwind(|| {
        execution_finishes_threads(ver, action);
    });
    assert_counterexample(action, result, "Err");
}

#[test]
#[serial]
fn verify_finishes_threads_and_monitor_returns_err() {
    execution_finishes_threads_and_monitor_returns_err(true);
}

#[test]
#[serial]
#[ignore] // TODO: test doesn't pass yet because error file is missing.
fn estimate_finishes_threads_and_monitor_returns_err() {
    execution_finishes_threads_and_monitor_returns_err(false);
}

fn execution_finishes_threads_and_monitor_panics(ver: bool) {
    let action = Action::MonitorPanics;
    let result = std::panic::catch_unwind(|| {
        execution_finishes_threads(ver, action);
    });
    assert_counterexample(action, result, "Expected Monitor Panic");
}

#[test]
#[serial]
fn verify_finishes_threads_and_monitor_panics() {
    execution_finishes_threads_and_monitor_panics(true);
}

#[test]
#[serial]
fn estimate_finishes_threads_and_monitor_panics() {
    execution_finishes_threads_and_monitor_panics(false);
}

fn execution_finishes_threads_and_monitor_tries_to_spawn(ver: bool) {
    let action = Action::MonitorTriesToSpawn;
    let result = std::panic::catch_unwind(|| {
        execution_finishes_threads(ver, action);
    });
    assert_counterexample(action, result, "only inside a Must test");
}

#[test]
#[serial]
fn verify_finishes_threads_and_monitor_tries_to_spawn() {
    execution_finishes_threads_and_monitor_tries_to_spawn(true);
}

#[test]
#[serial]
fn estimate_finishes_threads_and_monitor_tries_to_spawn() {
    execution_finishes_threads_and_monitor_tries_to_spawn(false);
}

fn execution_deadlocks_and_monitor_returns_ok(ver: bool) {
    let action = Action::MonitorReturnsOk;
    let result = std::panic::catch_unwind(|| {
        execution_deadlocks(ver, action);
    });
    assert_counterexample(action, result, "Ok");
}

#[test]
#[serial]
fn verify_deadlocks_and_monitor_returns_ok() {
    execution_deadlocks_and_monitor_returns_ok(true);
}

#[test]
#[serial]
fn estimate_deadlocks_and_monitor_returns_ok() {
    execution_deadlocks_and_monitor_returns_ok(false);
}

fn execution_deadlocks_and_monitor_returns_err(ver: bool) {
    let action = Action::MonitorReturnsErr;
    let result = std::panic::catch_unwind(|| {
        execution_deadlocks(ver, action);
    });
    assert_counterexample(action, result, "Err");
}

#[test]
#[serial]
fn verify_deadlocks_and_monitor_returns_err() {
    execution_deadlocks_and_monitor_returns_err(true);
}

#[test]
#[serial]
#[ignore] // TODO: fix. Test not yet passing because error file is missing.
fn estimate_deadlocks_and_monitor_returns_err() {
    execution_deadlocks_and_monitor_returns_err(false);
}

fn execution_deadlocks_and_monitor_panics(ver: bool) {
    let action = Action::MonitorPanics;
    let result = std::panic::catch_unwind(|| execution_deadlocks(ver, action));
    assert_counterexample(action, result, "Expected Monitor Panic");
}

#[test]
#[serial]
fn verify_deadlocks_and_monitor_panics() {
    execution_deadlocks_and_monitor_panics(true);
}

#[test]
#[serial]
fn estimate_deadlocks_and_monitor_panics() {
    execution_deadlocks_and_monitor_panics(false);
}

fn execution_deadlocks_and_monitor_tries_to_spawn(ver: bool) {
    let action = Action::MonitorTriesToSpawn;
    let result = std::panic::catch_unwind(|| execution_deadlocks(ver, action));
    assert_counterexample(action, result, "only inside a Must test");
}

#[test]
#[serial]
fn verify_deadlocks_and_monitor_tries_to_spawn() {
    execution_deadlocks_and_monitor_tries_to_spawn(true);
}

#[test]
#[serial]
fn estimate_deadlocks_and_monitor_tries_to_spawn() {
    execution_deadlocks_and_monitor_tries_to_spawn(false);
}

fn execution_assumes_false_and_monitor_not_executed(ver: bool) {
    remove_old_files();
    verify_or_estimate(ver, move || {
        let _ = start_monitor_action_monitor(ActionMonitor::new(Action::MonitorPanics));
        publish(PublishedThreadId {
            thread_id: current_id(),
        });
        send_msg(current_id(), current_id()); // Allow monitor to observe message
        assume!(false);
    });
    // Note: no panic! Because the monitor's on_stop was not invoked.
}

#[test]
#[serial]
fn verify_assumes_false_and_monitor_not_executed() {
    execution_assumes_false_and_monitor_not_executed(true);
}

#[test]
#[serial]
fn estimate_assumes_false_and_monitor_not_executed() {
    execution_assumes_false_and_monitor_not_executed(false);
}

fn execution_asserts_false_and_monitor_not_executed(ver: bool) {
    remove_old_files();
    let result = std::panic::catch_unwind(|| {
        verify_or_estimate(ver, move || {
            let _ = start_monitor_action_monitor(ActionMonitor::new(Action::MonitorPanics));
            publish(PublishedThreadId {
                thread_id: current_id(),
            });
            send_msg(current_id(), current_id()); // Allow monitor to observe message
            assert(false);
        });
    });
    assert_panic_contains(result, "assertion failed: cond");
}

#[test]
#[serial]
fn verify_asserts_false_and_monitor_not_executed() {
    execution_asserts_false_and_monitor_not_executed(true);
}

#[test]
#[serial]
fn estimate_asserts_false_and_monitor_not_executed() {
    execution_asserts_false_and_monitor_not_executed(false);
}

fn assert_counterexample(
    action: Action,
    result: Result<(), Box<dyn Any + Send>>,
    expected_msg: &str,
) {
    match action {
        Action::MonitorReturnsOk => {
            assert!(result.is_ok(), "verify() should not panic!");
            assert!(
                std::fs::metadata(ERROR_FILE).is_err(),
                "error file exists even though no counterexample was expected"
            );
        }
        Action::MonitorReturnsErr => {
            assert_panic_contains(result, expected_msg);
            assert!(
                std::fs::metadata(ERROR_FILE).is_ok(),
                "error file is missing"
            );
        }
        Action::MonitorPanics | Action::MonitorTriesToSpawn => {
            assert_panic_contains(result, expected_msg);
            assert!(
                std::fs::metadata(ERROR_FILE).is_ok(),
                "error file is missing"
            );
        }
    }
}

#[monitor(i32)]
#[derive(Clone, Debug, Default)]
pub struct MyBuggyMonitor;

impl Acceptor<i32> for MyBuggyMonitor {}

impl Observer<i32> for MyBuggyMonitor {}

impl Monitor for MyBuggyMonitor {}

#[test]
#[serial]
#[should_panic(expected = "Monitors can only be spawned from the main thread")]
fn unordered_spawn() {
    for _ in 0..TEST_RUNS {
        let stats = crate::verify(
            crate::Config::builder()
                .with_policy(crate::SchedulePolicy::Arbitrary)
                .with_cons_type(ConsType::FIFO)
                .build(),
            || {
                let _ = thread::spawn(|| {
                    crate::send_msg(main_thread_id(), 42);
                });
                let _ = thread::spawn(|| {
                    let mtid = start_monitor_my_buggy_monitor(MyBuggyMonitor {});
                    terminate_monitor_my_buggy_monitor(mtid.thread().id());
                    let _ = mtid.join().unwrap();
                });
            },
        );
        assert_eq!(stats.execs, 1);
    }
}
