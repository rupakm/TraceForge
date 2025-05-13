use crate::thread::{construct_thread_id, ThreadId};
use crate::{event::Event, indexed_map::IndexedMap};
use std::cmp;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct VectorClock {
    clock: IndexedMap<u32>,
}

impl VectorClock {
    pub(crate) fn new() -> Self {
        Self {
            clock: IndexedMap::new(),
        }
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = (ThreadId, u32)> + '_ {
        self.clock
            .enumerate()
            .map(|(tid, &idx)| (construct_thread_id(tid as u32), idx))
    }

    pub(crate) fn get(&self, i: ThreadId) -> Option<u32> {
        self.clock.get(usize::from(i)).copied()
    }

    // Populate tid and set index to 0
    pub(crate) fn set_tid(&mut self, tid: ThreadId) {
        self.clock.set(usize::from(tid), 0);
    }

    pub(crate) fn set(&mut self, e: Event) {
        self.clock.set(usize::from(e.thread), e.index);
    }

    // returns true iff self is a vector clock which represents a view that contains the event e; this is used for the "program order" vector clocks hb
    pub(crate) fn contains(&self, e: Event) -> bool {
        self.get(e.thread).is_some_and(|i| e.index <= i)
    }

    // Unchecked update (assumes that e.thread is present)
    pub(crate) fn update_idx(&mut self, e: Event) {
        self.clock[usize::from(e.thread)] = e.index;
    }

    // Update, populating the thread with 0, if it's missing
    pub(crate) fn update_or_set(&mut self, e: Event) {
        self.advance(usize::from(e.thread), e.index);
    }

    // Update with another vector
    pub(crate) fn update(&mut self, other: &Self) {
        for (tid, &other_val) in other.clock.enumerate() {
            self.advance(tid, other_val);
        }
    }

    // Advace tid to be at least ind, populating the tid entry if missing
    fn advance(&mut self, tid: usize, ind: u32) {
        let new_val: u32 = cmp::max(*self.clock.get(tid).unwrap_or(&0), ind);
        self.clock.set(tid, new_val);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::thread::construct_thread_id;

    impl From<u32> for ThreadId {
        fn from(tid: u32) -> Self {
            construct_thread_id(tid)
        }
    }

    /// This helper function accepts -1 as a value to show that a thread isn't present in the vc
    /// even though we don't allow this in the rest of the code because the index is unsigned
    /// in most other places.
    fn clock(value: &[i32]) -> VectorClock {
        let mut c = VectorClock::new();
        for (tid, &idx) in value.iter().enumerate() {
            if idx >= 0 {
                c.update_or_set(Event::new(ThreadId::from(tid as u32), idx.clone() as u32));
            }
        }
        c
    }

    #[test]
    fn vector_clock() {
        let mut v1: VectorClock = clock(&[1, 0, 2, 0]);
        v1.update_or_set(Event::new(ThreadId::from(1), 3));
        v1.update_or_set(Event::new(ThreadId::from(5), 5));
        assert_eq!(v1, clock(&[1, 3, 2, 0, -1, 5]));

        let mut v1 = clock(&[1]);
        v1.update_or_set(Event::new(ThreadId::from(3), 1));
        assert!(v1.contains(Event::new(ThreadId::from(3), 1)));
        assert!(!v1.contains(Event::new(ThreadId::from(2), 1)));

        let mut v1 = clock(&[1, -1, 2]);
        let v2 = clock(&[2, -1, 1, 5]);
        v1.update(&v2);
        assert_eq!(v1, clock(&[2, -1, 2, 5]));
    }

    #[test]
    fn vector_clock_is_sparse() {
        let mut c = clock(&[100]);
        c.update_or_set(Event::new(ThreadId::from(2), 1));
        assert_eq!(None, c.get(ThreadId::from(1)));
        assert_eq!(c, clock(&[100, -1, 1]));
    }

    #[test]
    fn vector_clock_is_serializable() {
        let c = clock(&[1, 2, 3]);
        let str = serde_json::to_string_pretty(&c).unwrap();
        let c2: VectorClock = serde_json::from_str(&str).unwrap();
        assert_eq!(c, c2);
    }
}
