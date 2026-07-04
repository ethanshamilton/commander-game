use super::state::PlannerState;
use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub usize);

pub type ConditionFn = fn(&PlannerState) -> bool;
pub type EffectFn = fn(&mut PlannerState);
pub type OperatorBindingFn = fn(&PlannerState) -> Option<BoundOperator>;

#[derive(Clone)]
pub enum Task {
    Primitive(PrimitiveTask),
    Compound(CompoundTask),
}

#[derive(Clone)]
pub struct PrimitiveTask {
    pub name: &'static str,
    pub reason: &'static str,
    pub preconditions: ConditionFn,
    pub bind: OperatorBindingFn,
    pub effects: EffectFn,
}

#[derive(Clone)]
pub struct CompoundTask {
    pub name: &'static str,
    /// Methods are stored in priority order. Lower index means higher priority.
    pub methods: Vec<Method>,
}

#[derive(Clone)]
pub struct Method {
    pub name: &'static str,
    pub preconditions: ConditionFn,
    pub subtasks: Vec<TaskId>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoundOperator {
    Hold,
    MoveTo { destination_m: Vec2 },
    FireAt { target: Entity },
}

impl BoundOperator {
    pub fn describe(&self) -> String {
        match self {
            BoundOperator::Hold => "hold position / hold fire".to_string(),
            BoundOperator::MoveTo { destination_m } => {
                format!("move to ({:.1}, {:.1})m", destination_m.x, destination_m.y)
            }
            BoundOperator::FireAt { target } => format!("fire at {target:?}"),
        }
    }
}

pub struct Domain {
    pub tasks: Vec<Task>,
    pub root: TaskId,
}

impl Domain {
    pub fn task(&self, id: TaskId) -> Option<&Task> {
        self.tasks.get(id.0)
    }

    pub fn task_name(&self, id: TaskId) -> Option<&'static str> {
        match self.task(id)? {
            Task::Primitive(task) => Some(task.name),
            Task::Compound(task) => Some(task.name),
        }
    }
}

#[derive(Default)]
pub struct DomainBuilder {
    tasks: Vec<Task>,
}

impl DomainBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn primitive(
        &mut self,
        name: &'static str,
        preconditions: ConditionFn,
        bind: OperatorBindingFn,
        effects: EffectFn,
    ) -> TaskId {
        self.primitive_with_reason(
            name,
            "selected by HTN decomposition",
            preconditions,
            bind,
            effects,
        )
    }

    pub fn primitive_with_reason(
        &mut self,
        name: &'static str,
        reason: &'static str,
        preconditions: ConditionFn,
        bind: OperatorBindingFn,
        effects: EffectFn,
    ) -> TaskId {
        let id = TaskId(self.tasks.len());
        self.tasks.push(Task::Primitive(PrimitiveTask {
            name,
            reason,
            preconditions,
            bind,
            effects,
        }));
        id
    }

    pub fn compound(&mut self, name: &'static str, methods: Vec<Method>) -> TaskId {
        let id = TaskId(self.tasks.len());
        self.tasks
            .push(Task::Compound(CompoundTask { name, methods }));
        id
    }

    pub fn build(self, root: TaskId) -> Domain {
        debug_assert!(root.0 < self.tasks.len(), "domain root task does not exist");
        Domain {
            tasks: self.tasks,
            root,
        }
    }
}

pub fn always(_state: &PlannerState) -> bool {
    true
}

pub fn never(_state: &PlannerState) -> bool {
    false
}

pub fn no_effect(_state: &mut PlannerState) {}

pub fn bind_hold(_state: &PlannerState) -> Option<BoundOperator> {
    Some(BoundOperator::Hold)
}

pub fn bind_fire_at_nearest_hostile(state: &PlannerState) -> Option<BoundOperator> {
    Some(BoundOperator::FireAt {
        target: state.nearest_hostile?.entity,
    })
}

pub fn bind_move_to_current_position(state: &PlannerState) -> Option<BoundOperator> {
    Some(BoundOperator::MoveTo {
        destination_m: state.position_m,
    })
}

pub fn bind_move_to_last_known_hostile_position(state: &PlannerState) -> Option<BoundOperator> {
    Some(BoundOperator::MoveTo {
        destination_m: state.nearest_hostile?.position_m,
    })
}

pub fn bind_move_30m_away_from_nearest_hostile(state: &PlannerState) -> Option<BoundOperator> {
    let hostile = state.nearest_hostile?;
    let away = (state.position_m - hostile.position_m).normalize_or_zero();
    Some(BoundOperator::MoveTo {
        destination_m: state.position_m + away * 30.0,
    })
}
