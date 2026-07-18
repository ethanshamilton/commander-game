use super::state::PlannerState;
use crate::ai::perception::ContactKind;
use crate::gameplay::combat::CombatOrder;
use crate::gameplay::missions::{MissionId, PendingTaskAssignment, TaskDirective};
use crate::gameplay::orders::{CombatOrderSource, UnitOrderSource};
use crate::gameplay::simulation::UnitOrder;
use bevy::prelude::*;

const MOVE_DESTINATION_EPSILON_M: f32 = 0.05;

/// A primitive task bound to concrete parameters, ready to be dispatched as
/// gameplay orders. Doctrine-level task names (see `domain.rs`) resolve into
/// these at plan time; the executor never reasons about task names directly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoundOperator {
    Hold,
    MoveTo {
        destination_m: Vec2,
    },
    FireAt {
        target: Entity,
    },
    DelegateHoldStation {
        mission_id: MissionId,
        mission_issued_tick: u64,
        assignee: Entity,
        station_m: Vec2,
        rally_point_m: Vec2,
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
            BoundOperator::MoveTo { destination_m } => {
                format!("move to ({:.1}, {:.1})m", destination_m.x, destination_m.y)
            }
            BoundOperator::FireAt { target } => format!("fire at {target:?}"),
            BoundOperator::DelegateHoldStation {
                assignee,
                station_m,
                ..
            } => format!(
                "delegate hold station ({:.1}, {:.1})m to {assignee:?}",
                station_m.x, station_m.y
            ),
        }
    }

    /// Issue the orders that realize this operator. All orders are tagged
    /// `OrderSource::Htn`.
    pub fn dispatch(&self, commands: &mut Commands, entity: Entity) {
        match *self {
            BoundOperator::Hold => {
                commands.entity(entity).insert((
                    UnitOrder::Hold,
                    UnitOrderSource::htn(),
                    CombatOrder::HoldFire,
                    CombatOrderSource::htn(),
                ));
            }
            BoundOperator::MoveTo { destination_m } => {
                commands
                    .entity(entity)
                    .insert((UnitOrder::MoveTo { destination_m }, UnitOrderSource::htn()));
            }
            BoundOperator::FireAt { target } => {
                commands
                    .entity(entity)
                    .insert((CombatOrder::FireAt { target }, CombatOrderSource::htn()));
            }
            BoundOperator::DelegateHoldStation {
                mission_id,
                mission_issued_tick,
                assignee,
                station_m,
                rally_point_m,
                expires_at,
            } => {
                commands.entity(entity).insert(PendingTaskAssignment {
                    mission_issued_tick,
                    assignee,
                    directive: TaskDirective::HoldStation {
                        mission_id,
                        station_m,
                        facing_radians: None,
                        rally_point_m,
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
        unit_order: Option<&UnitOrder>,
        combat_order: Option<&CombatOrder>,
    ) -> StepPoll {
        match *self {
            BoundOperator::Hold => StepPoll::Running,
            BoundOperator::MoveTo { destination_m } => poll_move(destination_m, unit_order),
            BoundOperator::FireAt { target } => poll_fire(target, combat_order, state),
            BoundOperator::DelegateHoldStation {
                mission_id,
                mission_issued_tick,
                assignee,
                ..
            } => {
                if state.has_delegated_to((mission_id, mission_issued_tick), assignee) {
                    StepPoll::Succeeded
                } else if state
                    .assigned_mission
                    .is_none_or(|mission| mission.identity() != (mission_id, mission_issued_tick))
                {
                    StepPoll::Failed("assigned mission changed during delegation")
                } else {
                    StepPoll::Running
                }
            }
        }
    }
}

fn poll_move(destination_m: Vec2, current_order: Option<&UnitOrder>) -> StepPoll {
    match current_order {
        None => StepPoll::Succeeded,
        Some(UnitOrder::MoveTo {
            destination_m: current,
        }) => {
            if current.distance(destination_m) <= MOVE_DESTINATION_EPSILON_M {
                StepPoll::Running
            } else {
                StepPoll::Failed("move order destination changed")
            }
        }
        Some(UnitOrder::Hold) => StepPoll::Failed("move order replaced by hold"),
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
            destination_m: Vec2::new(1.0, 0.0),
        };
        let state = PlannerState::default();

        assert_eq!(op.poll(&state, None, None), StepPoll::Succeeded);
    }

    #[test]
    fn move_keeps_running_while_destination_matches() {
        let destination_m = Vec2::new(1.0, 0.0);
        let op = BoundOperator::MoveTo { destination_m };
        let state = PlannerState::default();
        let order = UnitOrder::MoveTo { destination_m };

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
