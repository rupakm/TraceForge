//! Revisiting utilities

use serde::{Deserialize, Serialize};

use crate::event::Event;
use std::fmt::Debug;

/// Models the different possible revisit types.  These all carry the
/// same info, but Must needs to be able to distinguish among them
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum RevisitEnum {
    ForwardRevisit(Revisit),
    BackwardRevisit(Revisit),
}

impl RevisitEnum {
    /// forward revisit of pos (recv or send) with new placement (send)
    pub(crate) fn new_forward(pos: Event, placement: Event) -> Self {
        RevisitEnum::ForwardRevisit(Revisit {
            pos,
            rev: placement,
        })
    }

    /// backward revisit of recv by send
    pub(crate) fn new_backward(recv: Event, send: Event) -> Self {
        RevisitEnum::BackwardRevisit(Revisit {
            pos: recv,
            rev: send,
        })
    }

    fn get_revisit(&self) -> &Revisit {
        match self {
            RevisitEnum::ForwardRevisit(r) => r,
            RevisitEnum::BackwardRevisit(r) => r,
        }
    }
    pub(crate) fn pos(&self) -> Event {
        self.get_revisit().pos
    }

    pub(crate) fn rev(&self) -> Event {
        self.get_revisit().rev
    }
}

/// A revisit item to be examined by Must
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Revisit {
    /// the event whoce placement (rf or co choice) chages
    pub(crate) pos: Event,
    /// the placement (rf or co choice)
    pub(crate) rev: Event,
}

impl Revisit {
    pub(crate) fn new(pos: Event, rev: Event) -> Self {
        Self { pos, rev }
    }
}
