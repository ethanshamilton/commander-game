use super::domain::{BoundOperator, ConditionFn, Domain, Task, TaskId};
use super::state::{HostileBelief, PlannerState};
use bevy::prelude::*;

#[derive(Debug, Clone)]
pub struct Plan {
    pub steps: Vec<BoundStep>,
    pub mtr: Mtr,
}

impl Plan {
    pub fn describe(&self) -> String {
        let mut lines = vec![format!("MTR {:?}", self.mtr.0)];

        for (index, step) in self.steps.iter().enumerate() {
            lines.push(format!("{index}: {}", step.describe()));
        }

        lines.join("\n")
    }
}

#[derive(Debug, Clone)]
pub struct BoundStep {
    pub task: TaskId,
    pub task_name: &'static str,
    pub reason: &'static str,
    pub operator: BoundOperator,
    pub preconditions: ConditionFn,
}

impl BoundStep {
    pub fn describe(&self) -> String {
        format!(
            "{} [{:?}] -> {}",
            self.task_name,
            self.task,
            self.operator.describe()
        )
    }
}

/// Method traversal record.
///
/// Each entry is the method index chosen at one compound decomposition point.
/// Lower method indices are higher priority because domain method order is
/// doctrine order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mtr(pub Vec<usize>);

impl Mtr {
    pub fn outranks(&self, other: &Mtr) -> bool {
        for (a, b) in self.0.iter().zip(other.0.iter()) {
            if a != b {
                return a < b;
            }
        }

        self.0.len() < other.0.len()
    }
}

pub fn plan(domain: &Domain, state: &PlannerState) -> Option<Plan> {
    let mut sim_state = state.clone();
    let mut steps = Vec::new();
    let mut mtr = Vec::new();

    decompose_task(domain, domain.root, &mut sim_state, &mut steps, &mut mtr).then_some(Plan {
        steps,
        mtr: Mtr(mtr),
    })
}

fn decompose_task(
    domain: &Domain,
    task_id: TaskId,
    state: &mut PlannerState,
    steps: &mut Vec<BoundStep>,
    mtr: &mut Vec<usize>,
) -> bool {
    let Some(task) = domain.task(task_id) else {
        debug_assert!(false, "domain references missing task {task_id:?}");
        return false;
    };

    match task {
        Task::Primitive(primitive) => {
            if !(primitive.preconditions)(state) {
                return false;
            }

            let Some(operator) = (primitive.bind)(state) else {
                return false;
            };

            steps.push(BoundStep {
                task: task_id,
                task_name: primitive.name,
                reason: primitive.reason,
                operator,
                preconditions: primitive.preconditions,
            });
            (primitive.effects)(state);
            true
        }
        Task::Compound(compound) => {
            for (method_index, method) in compound.methods.iter().enumerate() {
                if !(method.preconditions)(state) {
                    continue;
                }

                let saved_state = state.clone();
                let saved_step_len = steps.len();
                let saved_mtr_len = mtr.len();

                mtr.push(method_index);

                let succeeded = method
                    .subtasks
                    .iter()
                    .copied()
                    .all(|subtask| decompose_task(domain, subtask, state, steps, mtr));

                if succeeded {
                    return true;
                }

                *state = saved_state;
                steps.truncate(saved_step_len);
                mtr.truncate(saved_mtr_len);
            }

            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::htn::domain::{
        DomainBuilder, Method, always, bind_fire_at_nearest_hostile, bind_hold,
        bind_move_to_current_position, no_effect,
    };
    use crate::ai::perception::ContactKind;
    use bevy::ecs::world::World;

    fn has_ammo(state: &PlannerState) -> bool {
        state.has_ammo
    }

    fn has_move_target(state: &PlannerState) -> bool {
        state.has_move_target
    }

    fn spend_ammo(state: &mut PlannerState) {
        state.has_ammo = false;
    }

    fn acquire_move_target(state: &mut PlannerState) {
        state.has_move_target = true;
    }

    #[test]
    fn plan_description_includes_mtr_steps_and_bound_operators() {
        let mut builder = DomainBuilder::new();
        let root = builder.primitive("Hold", always, bind_hold, no_effect);
        let domain = builder.build(root);

        let plan = plan(&domain, &PlannerState::default()).unwrap();
        let description = plan.describe();

        assert!(description.contains("MTR []"));
        assert!(description.contains("0: Hold [TaskId(0)] -> hold position / hold fire"));
    }

    #[test]
    #[ignore = "debug planner output"]
    fn print_sample_plan() {
        let mut world = World::new();
        let target = world.spawn_empty().id();
        let mut builder = DomainBuilder::new();
        let fire = builder.primitive("Fire", always, bind_fire_at_nearest_hostile, no_effect);
        let hold = builder.primitive("Hold", always, bind_hold, no_effect);
        let root = builder.compound(
            "Root",
            vec![
                Method {
                    name: "Engage",
                    preconditions: always,
                    subtasks: vec![fire],
                },
                Method {
                    name: "Idle",
                    preconditions: always,
                    subtasks: vec![hold],
                },
            ],
        );
        let domain = builder.build(root);
        let state = PlannerState {
            nearest_hostile: Some(HostileBelief {
                entity: target,
                position_m: Vec2::new(10.0, 0.0),
                confidence: 1.0,
                last_seen_tick: 1,
                kind: ContactKind::Visual,
            }),
            tick: 1,
            ..default()
        };

        let plan = plan(&domain, &state).unwrap();
        println!("{}", plan.describe());
    }

    #[test]
    fn primitive_root_plans_one_step() {
        let mut builder = DomainBuilder::new();
        let root = builder.primitive("Hold", always, bind_hold, no_effect);
        let domain = builder.build(root);

        let plan = plan(&domain, &PlannerState::default()).unwrap();

        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].task_name, "Hold");
        assert_eq!(plan.steps[0].operator, BoundOperator::Hold);
        assert_eq!(plan.mtr, Mtr(vec![]));
    }

    #[test]
    fn compound_chooses_first_valid_method() {
        let mut builder = DomainBuilder::new();
        let bad = builder.primitive("Bad", always, bind_hold, no_effect);
        let hold = builder.primitive("Hold", always, bind_hold, no_effect);
        let root = builder.compound(
            "Root",
            vec![
                Method {
                    name: "Invalid",
                    preconditions: crate::ai::htn::domain::never,
                    subtasks: vec![bad],
                },
                Method {
                    name: "Valid",
                    preconditions: always,
                    subtasks: vec![hold],
                },
            ],
        );
        let domain = builder.build(root);

        let plan = plan(&domain, &PlannerState::default()).unwrap();

        assert_eq!(plan.steps[0].task_name, "Hold");
        assert_eq!(plan.mtr, Mtr(vec![1]));
    }

    #[test]
    fn method_order_is_priority_order() {
        let mut builder = DomainBuilder::new();
        let hold = builder.primitive("Hold", always, bind_hold, no_effect);
        let mov = builder.primitive("Move", always, bind_move_to_current_position, no_effect);
        let root = builder.compound(
            "Root",
            vec![
                Method {
                    name: "HigherPriority",
                    preconditions: always,
                    subtasks: vec![hold],
                },
                Method {
                    name: "LowerPriority",
                    preconditions: always,
                    subtasks: vec![mov],
                },
            ],
        );
        let domain = builder.build(root);

        let plan = plan(&domain, &PlannerState::default()).unwrap();

        assert_eq!(plan.steps[0].task_name, "Hold");
        assert_eq!(plan.mtr, Mtr(vec![0]));
    }

    #[test]
    fn failed_later_subtask_backtracks_steps_mtr_and_state() {
        let mut builder = DomainBuilder::new();
        let spend = builder.primitive("SpendAmmo", has_ammo, bind_hold, spend_ammo);
        let fire = builder.primitive("Fire", has_ammo, bind_hold, no_effect);
        let hold = builder.primitive("Hold", always, bind_hold, no_effect);
        let root = builder.compound(
            "Root",
            vec![
                Method {
                    name: "Doomed",
                    preconditions: always,
                    subtasks: vec![spend, fire],
                },
                Method {
                    name: "Fallback",
                    preconditions: always,
                    subtasks: vec![hold],
                },
            ],
        );
        let domain = builder.build(root);
        let state = PlannerState {
            has_ammo: true,
            ..default()
        };

        let plan = plan(&domain, &state).unwrap();

        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].task_name, "Hold");
        assert_eq!(plan.mtr, Mtr(vec![1]));
    }

    #[test]
    fn primitive_effects_influence_later_subtasks() {
        let mut builder = DomainBuilder::new();
        let acquire =
            builder.primitive("AcquireMoveTarget", always, bind_hold, acquire_move_target);
        let mov = builder.primitive(
            "Move",
            has_move_target,
            bind_move_to_current_position,
            no_effect,
        );
        let root = builder.compound(
            "Root",
            vec![Method {
                name: "MoveAfterAcquire",
                preconditions: always,
                subtasks: vec![acquire, mov],
            }],
        );
        let domain = builder.build(root);

        let plan = plan(&domain, &PlannerState::default()).unwrap();

        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].task_name, "AcquireMoveTarget");
        assert_eq!(plan.steps[1].task_name, "Move");
    }

    #[test]
    fn no_valid_plan_returns_none() {
        let mut builder = DomainBuilder::new();
        let root = builder.primitive(
            "Impossible",
            crate::ai::htn::domain::never,
            bind_hold,
            no_effect,
        );
        let domain = builder.build(root);

        assert!(plan(&domain, &PlannerState::default()).is_none());
    }

    #[test]
    fn mtr_outranking_is_lexicographic_with_lower_method_indices_first() {
        assert!(Mtr(vec![0]).outranks(&Mtr(vec![1])));
        assert!(Mtr(vec![0, 0]).outranks(&Mtr(vec![0, 1])));
        assert!(Mtr(vec![0]).outranks(&Mtr(vec![0, 1])));
        assert!(!Mtr(vec![1]).outranks(&Mtr(vec![0])));
        assert!(!Mtr(vec![0, 1]).outranks(&Mtr(vec![0, 1])));
    }

    #[test]
    fn binding_fire_at_nearest_hostile_uses_hostile_entity() {
        let mut world = World::new();
        let target = world.spawn_empty().id();
        let mut builder = DomainBuilder::new();
        let fire = builder.primitive("Fire", always, bind_fire_at_nearest_hostile, no_effect);
        let hold = builder.primitive("Hold", always, bind_hold, no_effect);
        let root = builder.compound(
            "Root",
            vec![
                Method {
                    name: "Engage",
                    preconditions: always,
                    subtasks: vec![fire],
                },
                Method {
                    name: "Idle",
                    preconditions: always,
                    subtasks: vec![hold],
                },
            ],
        );
        let domain = builder.build(root);
        let state = PlannerState {
            nearest_hostile: Some(HostileBelief {
                entity: target,
                position_m: Vec2::new(10.0, 0.0),
                confidence: 1.0,
                last_seen_tick: 1,
                kind: ContactKind::Visual,
            }),
            tick: 1,
            ..default()
        };

        let plan = plan(&domain, &state).unwrap();

        assert_eq!(plan.steps[0].operator, BoundOperator::FireAt { target });
        assert_eq!(plan.mtr, Mtr(vec![0]));
    }

    #[test]
    fn failed_operator_binding_backtracks_to_later_method() {
        let mut builder = DomainBuilder::new();
        let fire = builder.primitive("Fire", always, bind_fire_at_nearest_hostile, no_effect);
        let hold = builder.primitive("Hold", always, bind_hold, no_effect);
        let root = builder.compound(
            "Root",
            vec![
                Method {
                    name: "Engage",
                    preconditions: always,
                    subtasks: vec![fire],
                },
                Method {
                    name: "Idle",
                    preconditions: always,
                    subtasks: vec![hold],
                },
            ],
        );
        let domain = builder.build(root);

        let plan = plan(&domain, &PlannerState::default()).unwrap();

        assert_eq!(plan.steps[0].task_name, "Hold");
        assert_eq!(plan.mtr, Mtr(vec![1]));
    }
}
