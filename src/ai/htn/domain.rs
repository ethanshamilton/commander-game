pub use super::operators::BoundOperator;
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
        validate_domain(&self.tasks, root);
        Domain {
            tasks: self.tasks,
            root,
        }
    }
}

fn validate_domain(tasks: &[Task], root: TaskId) {
    assert!(root.0 < tasks.len(), "domain root task does not exist");

    for (task_index, task) in tasks.iter().enumerate() {
        let Task::Compound(compound) = task else {
            continue;
        };

        for method in &compound.methods {
            for subtask in &method.subtasks {
                assert!(
                    subtask.0 < tasks.len(),
                    "domain task {task_index} method {} references missing subtask {:?}",
                    method.name,
                    subtask
                );
            }
        }
    }

    let mut visiting = vec![false; tasks.len()];
    let mut visited = vec![false; tasks.len()];
    detect_cycle(tasks, root, &mut visiting, &mut visited);
}

fn detect_cycle(tasks: &[Task], task_id: TaskId, visiting: &mut [bool], visited: &mut [bool]) {
    if visited[task_id.0] {
        return;
    }

    assert!(
        !visiting[task_id.0],
        "domain contains a task cycle at {task_id:?}"
    );
    visiting[task_id.0] = true;

    if let Task::Compound(compound) = &tasks[task_id.0] {
        for method in &compound.methods {
            for subtask in &method.subtasks {
                detect_cycle(tasks, *subtask, visiting, visited);
            }
        }
    }

    visiting[task_id.0] = false;
    visited[task_id.0] = true;
}

pub fn always(_state: &PlannerState) -> bool {
    true
}

#[cfg(test)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "domain root task does not exist")]
    fn build_rejects_missing_root() {
        DomainBuilder::new().build(TaskId(0));
    }

    #[test]
    #[should_panic(expected = "references missing subtask")]
    fn build_rejects_missing_subtask() {
        let mut builder = DomainBuilder::new();
        let root = builder.compound(
            "Root",
            vec![Method {
                name: "Bad",
                preconditions: always,
                subtasks: vec![TaskId(99)],
            }],
        );

        builder.build(root);
    }

    #[test]
    #[should_panic(expected = "domain contains a task cycle")]
    fn build_rejects_task_cycles() {
        let mut builder = DomainBuilder::new();
        let root = builder.compound(
            "Root",
            vec![Method {
                name: "Self",
                preconditions: always,
                subtasks: vec![TaskId(0)],
            }],
        );

        builder.build(root);
    }
}
