use crate::msg::Message;
use crate::thread::ThreadId;
use crate::Val;
use std::any::TypeId;
use std::collections::BTreeMap;
use std::marker::PhantomData;

pub type MonitorError = String;
pub type MonitorResult = Result<(), MonitorError>;

pub trait Observer<M: Send + std::fmt::Debug + Message> {
    fn notify(&mut self, _who: ThreadId, _whom: ThreadId, _what: &M) -> MonitorResult {
        Ok(())
    }
}

pub trait Acceptor<M: Send + std::fmt::Debug + Message> {
    fn accept(&mut self, _who: ThreadId, _whom: ThreadId, _what: &M) -> bool {
        true
    }
}

/// Must Monitors are used to check specifications.
///
/// A monitor is a Rust implementation of a state machine that consumes the messages
/// of the application to decide whether a specification is met.
///
/// A monitor can implement the `Acceptor<T>` trait to specify which messages it
/// observes, and then implement `Observer<T>` to mutate its own state.
///
/// If a monitor panics, this is treated as a counterexample, so Monitors can use
/// rust `assert!` and similar things.
///
/// A monitor also has an on_stop function which checks specifications at the end of
/// the execution.
pub trait Monitor {
    /// A monitor's on_stop function is executed once at the end of each concrete execution
    /// explored. Use this to check for any invariants which should be true at the end
    /// of each exploration.
    ///
    /// Note that if the execution ends due to a false assumption, the monitor's on_stop
    /// is NOT invoked, since the purpose of assume() is to specify bounds within which
    /// the analysis of the model checker is valid.
    ///
    /// If you want to have a way to measure executions which are terminated due to an
    /// assumption, the Must telemetry module can be used to do this.
    fn on_stop(&mut self, _execution_end: &ExecutionEnd) -> MonitorResult {
        Ok(())
    }
}

pub struct ExecutionEnd<'a> {
    pub condition: EndCondition,
    pub(crate) published_values: BTreeMap<(ThreadId, TypeId), Val>,
    // We don't need the lifetime anymore, but we're not
    // removing it yet for the sake of backwards compatibility.
    pub(crate) _unused_lifetime: PhantomData<&'a ()>,
}

/// Describes the condition under which a monitor's on_stop function is invoked
#[derive(Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub enum EndCondition {
    /// The monitor was terminated before the execution ended
    MonitorTerminated,
    /// The execution ended with all non-daemon threads finishing and exiting.
    AllThreadsCompleted,
    /// The execution ended with at least one non-daemon thread blocking on
    /// recv_msg_block or JoinHandle.join
    Deadlock,
    /// The execution was discontinued because of a false assumption or false assertion.
    /// When an assumption is violated, the monitor's on_stop is not invoked, but
    /// telemetry for the execution will still be computed.
    FailedAssumption,
}

impl ExecutionEnd<'_> {
    pub fn get_published<T>(&self) -> BTreeMap<ThreadId, T>
    where
        T: Message + Clone + 'static,
    {
        let mut map: BTreeMap<ThreadId, T> = BTreeMap::new();
        for ((thread_id, _), boxed) in self.published_values.iter() {
            if let Some(val) = boxed.as_any_ref().downcast_ref::<T>() {
                map.insert(*thread_id, val.clone());
            }
        }
        map
    }
}
