use log::info;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::event_label::{AsEventLabel, LabelEnum};
use crate::must::MustState;
use crate::{event::*, Config, ThreadId};

/// The topologically sorted execution graph represents a linearization
/// of an execution.
/// `label_order`: It contains a vector of labels that describe the
/// action performed. The order of the vector represents the total order
/// of these actions.
/// `labels`: This is needed to ensure there are no duplicates in the linearization.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct TopologicallySortedExecutionGraph {
    label_order: VecDeque<LabelEnum>,
    labels: HashSet<Event>,
}

impl TopologicallySortedExecutionGraph {
    pub(crate) fn new() -> Self {
        TopologicallySortedExecutionGraph {
            label_order: VecDeque::new(),
            labels: HashSet::new(),
        }
    }

    /// Add a new label to the linearization.
    pub(crate) fn insert_label(&mut self, label: LabelEnum) {
        // Prevent duplicates
        if !self.labels.contains(&label.pos()) {
            info!("Adding {} to graph", label);

            // Add label to the `label_order`
            // and its unique id to `labels`
            self.labels.insert(label.pos());
            self.label_order.push_back(label);
        }
    }

    /// Obtain the next task to be executed
    /// `current_event`: If the current event has not been scheduled yet,
    /// simply return it.
    /// The next task cannot be scheduled until the `current_event` has been replayed
    /// and has value `None`.
    pub(crate) fn next_task(&mut self, current_event: Option<LabelEnum>) -> Option<LabelEnum> {
        if current_event.is_some() {
            return current_event;
        }
        let event = self.label_order.pop_front();
        match event {
            None => {
                info!("End of trace!");
                None
            }
            Some(label) => match label {
                LabelEnum::Begin(b) => {
                    info!("Skipping: {}", b);
                    // Skip begins as they are already replayed when a thread begins
                    self.next_task(current_event)
                }
                LabelEnum::End(e) => {
                    info!("Skipping: {}", e);
                    // Skip ends as they are already replayed when a thread ends
                    self.next_task(current_event)
                }
                _ => {
                    info!("Replaying: {}", label);
                    Some(label)
                }
            },
        }
    }

    pub(crate) fn filter(&self) -> TurmoilExecutionTrace {
        let mut v = Vec::new();
        let mut event_to_msg = HashMap::new();

        for e in self.label_order.iter() {
            if let LabelEnum::SendMsg(s) = e {
                let event_id = s.pos();
                // TODO: Anything to do with monitor values?
                let msg_string = format!("{:?}", s.val());
                event_to_msg.insert(event_id, msg_string);
            }

            if let LabelEnum::RecvMsg(r) = e {
                let sender = r.sender().to_number() as usize;
                let receiver = r.receiver().to_number() as usize;
                let sender_event = r.rf().unwrap();
                let msg = event_to_msg
                    .remove(&sender_event)
                    .expect("Could not find corresponding sender event");
                v.push((sender, receiver, msg));
            }
        }

        TurmoilExecutionTrace(v)
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct TurmoilExecutionTrace(Vec<(usize, usize, String)>);

impl std::fmt::Display for TopologicallySortedExecutionGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out_string = String::new();

        for label in self.label_order.iter() {
            let str = format!("{}", label);
            out_string.push_str(&str);
            out_string.push('\n');
        }
        write!(f, "{}", out_string)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ReplayInformation {
    // linearization of the execution graph
    sorted_error_graph: Option<TopologicallySortedExecutionGraph>,
    // clone the MustState when an error is detected
    error_state: Option<MustState>,
    // ensure that only the first error is recorded
    error_found: bool,
    // needed to ensure that a thread is scheduled until the current_event is replayed
    current_event: Option<LabelEnum>,
    // is mode of execution replay or not
    replay_mode: bool,
    // configuration parameters
    config: Config,
}

impl ReplayInformation {
    pub(crate) fn new(config: Config, replay_mode: bool) -> Self {
        ReplayInformation {
            sorted_error_graph: None,
            error_state: None,
            error_found: false,
            current_event: None,
            replay_mode,
            config,
        }
    }

    pub(crate) fn create(
        sorted_error_graph: TopologicallySortedExecutionGraph,
        error_state: MustState,
        config: Config,
    ) -> Self {
        ReplayInformation {
            sorted_error_graph: Some(sorted_error_graph),
            error_state: Some(error_state),
            error_found: true,
            current_event: None,
            replay_mode: true,
            config,
        }
    }

    pub(crate) fn extract_error_state(&mut self) -> MustState {
        assert!(self.error_state.is_some());

        self.error_state.take().unwrap()
    }

    pub(crate) fn error_found(&self) -> bool {
        self.error_found
    }

    pub(crate) fn current_event(&self) -> Option<LabelEnum> {
        self.current_event.clone()
    }

    pub(crate) fn reset_current_event(&mut self) {
        self.current_event = None;
    }

    pub(crate) fn sorted_error_graph(&self) -> &TopologicallySortedExecutionGraph {
        self.sorted_error_graph.as_ref().unwrap()
    }

    pub(crate) fn replay_mode(&self) -> bool {
        self.replay_mode
    }

    pub(crate) fn next_task(&mut self) -> Option<ThreadId> {
        assert!(self.sorted_error_graph.is_some());
        let next_label = self
            .sorted_error_graph
            .as_mut()
            .unwrap()
            .next_task(self.current_event.clone());

        match next_label {
            None => {
                self.current_event = None;
                None
            }
            Some(label) => {
                let task = label.thread();
                self.current_event = Some(label);
                Some(task)
            }
        }
    }

    pub(crate) fn config(&self) -> Config {
        self.config.clone()
    }
}
