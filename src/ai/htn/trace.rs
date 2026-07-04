use super::planner::Mtr;
use bevy::prelude::*;
use std::collections::VecDeque;

pub const DEFAULT_TRACE_CAPACITY: usize = 64;

#[derive(Component, Debug, Clone)]
pub struct DecisionTrace {
    events: VecDeque<TraceEvent>,
    capacity: usize,
}

impl Default for DecisionTrace {
    fn default() -> Self {
        Self::new(DEFAULT_TRACE_CAPACITY)
    }
}

impl DecisionTrace {
    pub fn new(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, event: TraceEvent) {
        if self.capacity == 0 {
            return;
        }

        while self.events.len() >= self.capacity {
            self.events.pop_front();
        }

        debug!(?event, "htn trace");
        self.events.push_back(event);
    }

    pub fn events(&self) -> impl Iterator<Item = &TraceEvent> {
        self.events.iter()
    }

    pub fn latest(&self) -> Option<&TraceEvent> {
        self.events.back()
    }
}

#[derive(Debug, Clone)]
pub enum TraceEvent {
    PlanCreated {
        root: &'static str,
        mtr: Mtr,
        steps: Vec<String>,
    },
    PlanRejected {
        reason: PlanRejectionReason,
    },
    StepStarted {
        task: &'static str,
        why: &'static str,
        operator: String,
    },
    StepFailed {
        task: &'static str,
        failed_condition: &'static str,
    },
    Replanned {
        trigger: ReplanTrigger,
    },
    PlanCompleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplanTrigger {
    NoPlan,
    PlanCompleted,
    StepFailed,
    RelevantStateChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanRejectionReason {
    NoValidPlan,
    MtrNotBetter,
    ExternalOrderActive,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_keeps_ring_capacity() {
        let mut trace = DecisionTrace::new(2);

        trace.push(TraceEvent::PlanRejected {
            reason: PlanRejectionReason::NoValidPlan,
        });
        trace.push(TraceEvent::Replanned {
            trigger: ReplanTrigger::NoPlan,
        });
        trace.push(TraceEvent::PlanCompleted);

        let events = trace.events().collect::<Vec<_>>();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], TraceEvent::Replanned { .. }));
        assert!(matches!(events[1], TraceEvent::PlanCompleted));
    }
}
