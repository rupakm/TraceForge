//! This file models a particular customer bug in which:
//! 1 - A monitor returns Err instead of using assert!
//! 2 - This happens in the on_stop function.
//! 3 - A counterexample should be generated but was not
//! 4 - When generated, the counterexample cannot be replayed

use std::fs;

use traceforge::{
    monitor_types::{ExecutionEnd, Monitor, MonitorResult},
    replay, verify, Config,
};
use traceforge_macros::monitor;

const TRACE_FILE: &str = "/tmp/bug_monitor_replay.rs.json";

fn scenario() {
    start_monitor_err_monitor(ErrMonitor {});
}

#[test]
fn test_generate_counterexample() {
    let _ = fs::remove_file(TRACE_FILE);
    let config = Config::builder().with_error_trace(TRACE_FILE).build();
    let result = std::panic::catch_unwind(|| {
        verify(config, scenario);
    });
    assert!(
        result.is_err(),
        "A failed monitor must generate a panic, even if that monitor itself did not panic"
    );
    assert!(
        fs::metadata(TRACE_FILE).is_ok(),
        "The trace file is missing"
    );
}

#[test]
fn test_replay_counterexample() {
    // test_generate_counterexample();
    let result = std::panic::catch_unwind(|| {
        replay(scenario, TRACE_FILE);
    });
    assert!(result.is_err(), "Replaying should generate a panic too");
}

#[monitor()]
#[derive(Clone, Debug, Default)]
struct ErrMonitor {}

impl Monitor for ErrMonitor {
    fn on_stop(&mut self, _execution_end: &ExecutionEnd) -> MonitorResult {
        MonitorResult::Err("Monitor not satisfied".to_string())
    }
}
