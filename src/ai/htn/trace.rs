use super::planner::Mtr;
use crate::gameplay::command_plans::CommandPlanId;
use bevy::prelude::*;
use std::collections::VecDeque;

pub const DEFAULT_TRACE_CAPACITY: usize = 64;

#[derive(Component, Debug, Clone)]
pub struct DecisionTrace {
    records: VecDeque<TraceRecord>,
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
            records: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, tick: u64, elapsed_s: f32, event: TraceEvent) {
        if self.capacity == 0 {
            return;
        }

        while self.records.len() >= self.capacity {
            self.records.pop_front();
        }

        debug!(tick, elapsed_s, ?event, "htn trace");
        self.records.push_back(TraceRecord {
            tick,
            elapsed_s,
            event,
        });
    }

    pub fn records(&self) -> impl DoubleEndedIterator<Item = &TraceRecord> {
        self.records.iter()
    }

    pub fn events(&self) -> impl Iterator<Item = &TraceEvent> {
        self.records.iter().map(|record| &record.event)
    }

    #[allow(dead_code)]
    pub fn latest(&self) -> Option<&TraceEvent> {
        self.records.back().map(|record| &record.event)
    }
}

#[derive(Debug, Clone)]
pub struct TraceRecord {
    pub tick: u64,
    pub elapsed_s: f32,
    pub event: TraceEvent,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
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
    CommandAssumed {
        predecessor: Entity,
        plan_id: Option<CommandPlanId>,
        squad_revision: u64,
    },
    RedelegationReset {
        plan_id: CommandPlanId,
        squad_revision: u64,
    },
    PlanCompleted,
}

#[allow(dead_code)]
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

        trace.push(
            1,
            0.1,
            TraceEvent::PlanRejected {
                reason: PlanRejectionReason::NoValidPlan,
            },
        );
        trace.push(
            2,
            0.2,
            TraceEvent::Replanned {
                trigger: ReplanTrigger::NoPlan,
            },
        );
        trace.push(3, 0.3, TraceEvent::PlanCompleted);

        let records = trace.records().collect::<Vec<_>>();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].tick, 2);
        assert_eq!(records[1].elapsed_s, 0.3);
        assert!(matches!(records[0].event, TraceEvent::Replanned { .. }));
        assert!(matches!(records[1].event, TraceEvent::PlanCompleted));
    }
}
