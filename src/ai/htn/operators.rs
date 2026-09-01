use super::state::PlannerState;
use crate::ai::perception::ContactKind;
use crate::gameplay::combat::CombatOrder;
use crate::gameplay::command_plans::{CommandPlanId, PendingTaskAssignment, TaskDirective};
use crate::gameplay::orders::{CombatOrderSource, MovementOrderSource};
use crate::gameplay::simulation::MovementOrder;
use crate::gameplay::spatial::PositionTarget;
use bevy::prelude::*;

const MOVE_DESTINATION_EPSILON_M: f32 = 0.05;

/// A primitive task bound to concrete parameters, ready to be dispatched as
/// gameplay orders. Doctrine-level task names (see `domain.rs`) resolve into
/// these at plan time; the executor never reasons about task names directly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoundOperator {
    Hold,
    MoveTo {
        target: PositionTarget,
    },
    FireAt {
        target: Entity,
    },
    DelegateHoldStation {
        plan_id: CommandPlanId,
        plan_issued_tick: u64,
        squad_revision: u64,
        assignee: Entity,
        station: PositionTarget,
        fallback: PositionTarget,
        expires_at: Option<u64>,
    },
}

/// Outcome of polling a running step's operator against current belief/order
/// state. `Running` means keep waiting; `Succeeded`/`Failed` both end the step,
/// differing only in the trace event/reason recorded by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepPoll {
    Running,
    Succeeded,
    Failed(&'static str),
}

impl BoundOperator {
    pub fn describe(&self) -> String {
        match self {
            BoundOperator::Hold => "hold position / hold fire".to_string(),
            BoundOperator::MoveTo { target } => match target.heading_radians {
                Some(heading) => format!(
                    "move to ({:.1}, {:.1})m facing {:.2}rad",
                    target.position_m.x, target.position_m.y, heading
                ),
                None => format!(
                    "move to ({:.1}, {:.1})m",
                    target.position_m.x, target.position_m.y
                ),
            },
            BoundOperator::FireAt { target } => format!("fire at {target:?}"),
            BoundOperator::DelegateHoldStation {
                assignee, station, ..
            } => format!(
                "delegate hold station ({:.1}, {:.1})m to {assignee:?}",
                station.position_m.x, station.position_m.y
            ),
        }
    }

    /// Issue the orders that realize this operator. All orders are tagged
    /// `OrderSource::Htn`.
    pub fn dispatch(&self, commands: &mut Commands, entity: Entity) {
        match *self {
            BoundOperator::Hold => {
                commands.entity(entity).insert((
                    MovementOrder::Hold,
                    MovementOrderSource::htn(),
                    CombatOrder::HoldFire,
                    CombatOrderSource::htn(),
                ));
            }
            BoundOperator::MoveTo { target } => {
                commands
                    .entity(entity)
                    .insert((MovementOrder::MoveTo { target }, MovementOrderSource::htn()));
            }
            BoundOperator::FireAt { target } => {
                commands
                    .entity(entity)
                    .insert((CombatOrder::FireAt { target }, CombatOrderSource::htn()));
            }
            BoundOperator::DelegateHoldStation {
                plan_id,
                plan_issued_tick,
                squad_revision,
                assignee,
                station,
                fallback,
                expires_at,
            } => {
                commands.entity(entity).insert(PendingTaskAssignment {
                    plan_issued_tick,
                    squad_revision,
                    assignee,
                    directive: TaskDirective::HoldStation {
                        plan_id,
                        station,
                        fallback,
                        expires_at,
                    },
                });
            }
        }
    }

    /// Check whether the running step finished, failed, or continues.
    pub fn poll(
        &self,
        state: &PlannerState,
        movement_order: Option<&MovementOrder>,
        combat_order: Option<&CombatOrder>,
    ) -> StepPoll {
        match *self {
            BoundOperator::Hold => StepPoll::Running,
            BoundOperator::MoveTo { target } => poll_move(target, movement_order),
            BoundOperator::FireAt { target } => poll_fire(target, combat_order, state),
            BoundOperator::DelegateHoldStation {
                plan_id,
                plan_issued_tick,
                squad_revision,
                assignee,
                ..
            } => {
                if state.command_squad_revision != Some(squad_revision) {
                    StepPoll::Failed("squad revision changed during delegation")
                } else if state.has_delegated_to((plan_id, plan_issued_tick), assignee) {
                    StepPoll::Succeeded
                } else if state
                    .assigned_plan
                    .is_none_or(|plan| plan.identity() != (plan_id, plan_issued_tick))
                {
                    StepPoll::Failed("assigned plan changed during delegation")
                } else {
                    StepPoll::Running
                }
            }
        }
    }
}

fn poll_move(target: PositionTarget, current_order: Option<&MovementOrder>) -> StepPoll {
    match current_order {
        None => StepPoll::Succeeded,
        Some(MovementOrder::MoveTo { target: current }) => {
            let same_position =
                current.position_m.distance(target.position_m) <= MOVE_DESTINATION_EPSILON_M;
            let same_heading = match (current.heading_radians, target.heading_radians) {
                (Some(current), Some(target)) => {
                    crate::gameplay::spatial::angular_distance(current, target)
                        <= MOVE_DESTINATION_EPSILON_M
                }
                (None, None) => true,
                _ => false,
            };
            if same_position && same_heading {
                StepPoll::Running
            } else {
                StepPoll::Failed("move order target changed")
            }
        }
        Some(MovementOrder::Hold) => StepPoll::Failed("move order replaced by hold"),
    }
}

fn poll_fire(target: Entity, combat_order: Option<&CombatOrder>, state: &PlannerState) -> StepPoll {
    match combat_order {
        Some(CombatOrder::FireAt { target: current }) if *current == target => {
            if !state.has_ammo {
                return StepPoll::Succeeded;
            }

            let Some(hostile) = state.nearest_hostile else {
                return StepPoll::Succeeded;
            };

            if hostile.entity != target
                || hostile.kind != ContactKind::Visual
                || !state.hostile_is_fresh(super::soldier::FRESH_CONTACT_TICKS)
            {
                return StepPoll::Succeeded;
            }

            StepPoll::Running
        }
        _ => StepPoll::Succeeded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::htn::state::HostileBelief;

    #[test]
    fn move_succeeds_when_order_removed() {
        let op = BoundOperator::MoveTo {
            target: PositionTarget::new(Vec2::new(1.0, 0.0), None),
        };
        let state = PlannerState::default();

        assert_eq!(op.poll(&state, None, None), StepPoll::Succeeded);
    }

    #[test]
    fn move_keeps_running_while_destination_matches() {
        let target = PositionTarget::new(Vec2::new(1.0, 0.0), Some(1.0));
        let op = BoundOperator::MoveTo { target };
        let state = PlannerState::default();
        let order = MovementOrder::MoveTo { target };

        assert_eq!(op.poll(&state, Some(&order), None), StepPoll::Running);
    }

    #[test]
    fn fire_succeeds_when_out_of_ammo() {
        let target = Entity::from_raw_u32(1).unwrap();
        let op = BoundOperator::FireAt { target };
        let state = PlannerState {
            has_ammo: false,
            nearest_hostile: Some(HostileBelief {
                entity: target,
                position_m: Vec2::ZERO,
                confidence: 1.0,
                last_seen_tick: 0,
                kind: ContactKind::Visual,
            }),
            ..Default::default()
        };
        let order = CombatOrder::FireAt { target };

        assert_eq!(op.poll(&state, None, Some(&order)), StepPoll::Succeeded);
    }

    #[test]
    fn fire_keeps_running_with_fresh_visual_hostile_and_ammo() {
        let target = Entity::from_raw_u32(1).unwrap();
        let op = BoundOperator::FireAt { target };
        let state = PlannerState {
            has_ammo: true,
            nearest_hostile: Some(HostileBelief {
                entity: target,
                position_m: Vec2::ZERO,
                confidence: 1.0,
                last_seen_tick: 0,
                kind: ContactKind::Visual,
            }),
            tick: 0,
            ..Default::default()
        };
        let order = CombatOrder::FireAt { target };

        assert_eq!(op.poll(&state, None, Some(&order)), StepPoll::Running);
    }
}
