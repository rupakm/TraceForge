use log::debug;

use crate::{monitor_types::EndCondition, telemetry::Coverage};

/// The coverage module allows users to place named coverage goals in their code, together with optional conditions.
/// The must engine gathers coverge information about how often a goal was hit, per execution and in aggregate.
/// Users can query coverage information by later checking if a coverage goal was covered (or how often it was covered in aggregate)
///
/// Example:
/// ```
/// use traceforge::{cover, thread, Config, Nondet, SchedulePolicy, TypeNondet};
///
/// const HERE: &'static str = "REACHED_HERE";
/// const MSG_IS_42: &'static str = "MSG_IS_42";
///
/// fn check_coverage() {
///     let stats = traceforge::verify(
///     Config::builder()
///     .with_policy(SchedulePolicy::Arbitrary)
///     .build(),
///     || {
///         let h = thread::spawn(move || {
///             let r: i32 = traceforge::recv_msg_block();
///             cover!(MSG_IS_42, r==42); // check that the message `42` was received on some run
///         });
///         if <bool>::nondet() {
///             traceforge::send_msg(h.thread().id(), 42);
///         } else {
///             cover!(HERE); // check that execution got here
///             traceforge::send_msg(h.thread().id(), 11);
///         }
///     });
///     assert_eq!(stats.execs, 2);
/// }
/// ```

#[derive(Default, Clone, Debug)]
pub struct CoverageInfo {
    // the field is currently public to implement merge easily
    // TODO: how can I do this without making the field public?
    pub coverage: Coverage,
}

impl CoverageInfo {
    pub fn new(coverage: Coverage) -> Self {
        Self { coverage }
    }

    pub fn is_covered(&self, goal: String) -> bool {
        *self.coverage.get(&goal).unwrap_or(&0) > 0
    }

    pub fn covered(&self, goal: String) -> u64 {
        *self.coverage.get(&goal).unwrap_or(&0)
    }

    pub fn merge(&mut self, rhs: &CoverageInfo) {
        for (k, v) in rhs.coverage.iter() {
            *self.coverage.get_mut(k).unwrap_or(&mut 0) += v;
        }
    }
}

impl From<Coverage> for CoverageInfo {
    fn from(coverage: Coverage) -> Self {
        Self { coverage }
    }
}

// For the moment, we use a `u32` to name each execution. In the future, we may want to hide the details into a type
pub type ExecutionId = u32;

// Callback functions that users can register to invoke after each run
pub trait ExecutionObserver {
    // this function is invoked at the beginning of each execution inside the model checker
    // this is often useful to initialize run-specific information
    fn before(&mut self, _eid: ExecutionId) {}

    // this function is invoked at the end of each execution inside the model checker
    // and gets execution information for that execution
    // The end condition states if this run completed normally or was blocked
    fn after(&mut self, _eid: ExecutionId, _end_condition: &EndCondition, _c: CoverageInfo) {}

    // this function is called by the model checker at the end of the exploration
    fn at_end_of_exploration(&mut self) {}
}

#[doc(hidden)] // should only be called from the `cover` macros
pub fn cover_goal(c: String, module: &'static str, file: &'static str, line: u32) {
    crate::ExecutionState::with(|s| {
        debug!("Covered {c}");
        let mut must = s.must.borrow_mut();
        let eid = must.telemetry.coverage.current_eid();
        must.telemetry.coverage.cover(eid, c, module, file, line);
    });
}

pub fn is_covered(c: String) -> bool {
    crate::ExecutionState::with(|s| {
        let must = s.must.borrow();
        must.telemetry.coverage.is_covered(c)
    })
}

pub fn covered(c: String) -> u64 {
    crate::ExecutionState::with(|s| {
        let must = s.must.borrow();
        must.telemetry.coverage.covered(c)
    })
}

#[macro_export]
macro_rules! cover {
    /* we initially allowed anonymous coverage goals `cover!()` but I think this is not a good idea, since
        the only way to refer to them later is by their location (file, line) and this seems very brittle.
        I am keeping this code until code review, in case we change our mind
    () => {
        let module = std::module_path!();
        let file = std::file!();
        let line = std::line!();
        $crate::coverage::cover_goal("<null>".into(), module, file, line); // format!("<null>::{file}::{line}"));
    };
    */
    ($name:expr) => {
        let module = std::module_path!();
        let file = std::file!();
        let line = std::line!();
        $crate::coverage::cover_goal($name.into(), module, file, line); //format!("{}::{file}::{line}", $name));
    };
    ($name:expr, $cond:expr $(,)?) => {
        // a coverage goal with a conditional only fires if the condition holds
        if $cond {
            let module = std::module_path!();
            let file = std::file!();
            let line = std::line!();
            $crate::coverage::cover_goal($name.into(), module, file, line); // format!("{}::{file}::{line}", $name));
        }
    };
}
