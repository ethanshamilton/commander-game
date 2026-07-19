use super::domain::{
    BoundOperator, DomainBuilder, Method, TaskId, always, bind_fire_at_nearest_hostile, bind_hold,
    bind_move_30m_away_from_nearest_hostile, bind_move_to_last_known_hostile_position,
};
use super::state::{AssignedTaskKind, PlannerState};
use crate::ai::perception::ContactKind;
use crate::gameplay::spatial::PositionTarget;

const LOW_HEALTH_FRAC: f32 = 0.35;
pub(crate) const FRESH_CONTACT_TICKS: u64 = 1;
const RECENT_CONTACT_TICKS: u64 = 100;
const MIN_ENGAGE_CONFIDENCE: f32 = 0.25;
const RETREAT_DISTANCE_M: f32 = 30.0;

/// Task handles installed into one domain builder. Both ordinary soldiers and
/// squad leaders reuse these tasks while choosing their own root priorities.
pub struct SoldierTasks {
    pub retreat: TaskId,
    pub fire: TaskId,
    pub execute_assigned_task: TaskId,
    pub investigate: TaskId,
    pub hold: TaskId,
}

pub fn add_soldier_tasks(builder: &mut DomainBuilder) -> SoldierTasks {
    let retreat = builder.primitive_with_reason(
        "MoveAwayFromNearestHostile",
        "health below 35% with hostile contact; survival outranks engagement",
        hostile_present,
        bind_move_30m_away_from_nearest_hostile,
        effect_move_away_from_hostile,
    );
    let fire = builder.primitive_with_reason(
        "FireAtNearestHostile",
        "fresh visual hostile contact and ammunition available",
        fresh_visual_hostile_with_ammo,
        bind_fire_at_nearest_hostile,
        no_fire_planner_effect,
    );
    let investigate = builder.primitive_with_reason(
        "MoveToLastKnownHostilePosition",
        "hostile contact is stale but recent; investigate last known position",
        hostile_present,
        bind_move_to_last_known_hostile_position,
        effect_move_to_last_known_hostile_position,
    );
    let hold = builder.primitive_with_reason(
        "Hold",
        "no higher-priority task is applicable",
        always,
        bind_hold,
        super::domain::no_effect,
    );
    let move_to_station = builder.primitive_with_reason(
        "MoveToAssignedStation",
        "assigned Hold Station task is active and the station has not been reached",
        assigned_task_needs_movement,
        bind_move_to_assigned_station,
        effect_arrive_at_assigned_station,
    );
    let hold_station = builder.primitive_with_reason(
        "HoldAssignedStation",
        "assigned Hold Station task is active and the station has been reached",
        at_active_assigned_station,
        bind_hold,
        super::domain::no_effect,
    );
    let execute_assigned_task = builder.compound(
        "ExecuteAssignedTask",
        vec![
            Method {
                name: "MoveToStation",
                preconditions: assigned_task_needs_movement,
                subtasks: vec![move_to_station],
            },
            Method {
                name: "HoldStation",
                preconditions: at_active_assigned_station,
                subtasks: vec![hold_station],
            },
        ],
    );

    SoldierTasks {
        retreat,
        fire,
        execute_assigned_task,
        investigate,
        hold,
    }
}

pub(crate) fn hostile_present(state: &PlannerState) -> bool {
    state.nearest_hostile.is_some()
}

pub(crate) fn low_health_with_hostile(state: &PlannerState) -> bool {
    state.health_frac < LOW_HEALTH_FRAC && hostile_present(state)
}

pub(crate) fn fresh_visual_hostile_with_ammo(state: &PlannerState) -> bool {
    let Some(hostile) = state.nearest_hostile else {
        return false;
    };

    state.has_ammo
        && hostile.kind == ContactKind::Visual
        && hostile.confidence >= MIN_ENGAGE_CONFIDENCE
        && state.hostile_is_fresh(FRESH_CONTACT_TICKS)
}

pub(crate) fn has_active_assigned_task(state: &PlannerState) -> bool {
    state.assigned_task.is_some_and(|task| {
        task.kind == AssignedTaskKind::HoldStation && !state.assigned_task_is_expired()
    })
}

fn assigned_task_needs_movement(state: &PlannerState) -> bool {
    has_active_assigned_task(state) && !state.at_assigned_station
}

fn at_active_assigned_station(state: &PlannerState) -> bool {
    has_active_assigned_task(state) && state.at_assigned_station
}

fn bind_move_to_assigned_station(state: &PlannerState) -> Option<BoundOperator> {
    Some(BoundOperator::MoveTo {
        target: state.assigned_task?.station,
    })
}

fn effect_arrive_at_assigned_station(state: &mut PlannerState) {
    let Some(task) = state.assigned_task else {
        return;
    };
    state.position_m = task.station.position_m;
    if let Some(heading) = task.station.heading_radians {
        state.heading_radians = heading;
    }
    state.at_assigned_station = true;
    state.has_move_target = true;
}

pub(crate) fn stale_recent_hostile(state: &PlannerState) -> bool {
    let Some(staleness) = state.hostile_staleness_ticks() else {
        return false;
    };

    staleness > FRESH_CONTACT_TICKS && staleness <= RECENT_CONTACT_TICKS
}

fn effect_move_away_from_hostile(state: &mut PlannerState) {
    let Some(hostile) = state.nearest_hostile else {
        return;
    };

    let away = (state.position_m - hostile.position_m).normalize_or_zero();
    state.position_m += away * RETREAT_DISTANCE_M;
    state.has_move_target = true;
}

fn effect_move_to_last_known_hostile_position(state: &mut PlannerState) {
    let Some(hostile) = state.nearest_hostile else {
        return;
    };

    state.position_m = hostile.position_m;
    state.has_move_target = true;
}

fn no_fire_planner_effect(_state: &mut PlannerState) {
    // Firing is stochastic and resolved by gameplay combat. The planner binds a
    // `FireAt` operator but does not pretend the hostile was killed, suppressed,
    // or otherwise changed in its simulated belief state.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::htn::domain::BoundOperator;
    use crate::ai::htn::leader::build_infantry_domain;
    use crate::ai::htn::planner::{Mtr, plan};
    use crate::ai::htn::state::{AssignedTaskBelief, HostileBelief};
    use crate::gameplay::command_plans::CommandPlanId;
    use bevy::prelude::*;

    fn state_with_hostile(kind: ContactKind, last_seen_tick: u64) -> PlannerState {
        let mut world = World::new();
        let hostile = world.spawn_empty().id();
        PlannerState {
            position_m: Vec2::ZERO,
            health_frac: 1.0,
            has_ammo: true,
            nearest_hostile: Some(HostileBelief {
                entity: hostile,
                position_m: Vec2::new(10.0, 0.0),
                confidence: 1.0,
                last_seen_tick,
                kind,
            }),
            under_fire: false,
            has_move_target: false,
            tick: 10,
            ..Default::default()
        }
    }

    #[test]
    fn no_hostile_idles() {
        let domain = build_infantry_domain();
        let plan = plan(&domain, &PlannerState::default()).unwrap();

        assert_eq!(plan.mtr, Mtr(vec![6]));
        assert_eq!(plan.steps[0].task_name, "Hold");
    }

    #[test]
    fn fresh_visual_hostile_engages() {
        let domain = build_infantry_domain();
        let state = state_with_hostile(ContactKind::Visual, 10);
        let plan = plan(&domain, &state).unwrap();

        assert_eq!(plan.mtr, Mtr(vec![1]));
        assert_eq!(plan.steps[0].task_name, "FireAtNearestHostile");
        assert!(matches!(
            plan.steps[0].operator,
            BoundOperator::FireAt { .. }
        ));
    }

    #[test]
    fn stale_recent_hostile_investigates() {
        let domain = build_infantry_domain();
        let state = state_with_hostile(ContactKind::Visual, 5);
        let plan = plan(&domain, &state).unwrap();

        assert_eq!(plan.mtr, Mtr(vec![5]));
        assert_eq!(plan.steps[0].task_name, "MoveToLastKnownHostilePosition");
        assert_eq!(
            plan.steps[0].operator,
            BoundOperator::MoveTo {
                target: PositionTarget::new(Vec2::new(10.0, 0.0), None)
            }
        );
    }

    #[test]
    fn assigned_hold_station_moves_then_holds_at_the_station() {
        let assigned_task = AssignedTaskBelief {
            plan_id: CommandPlanId(3),
            issued_tick: 7,
            kind: AssignedTaskKind::HoldStation,
            station: PositionTarget::new(Vec2::new(12.0, 4.0), Some(0.5)),
            fallback: PositionTarget::new(Vec2::ZERO, Some(0.5)),
            expires_at: None,
        };
        let moving = PlannerState {
            assigned_task: Some(assigned_task),
            ..Default::default()
        };
        let moving_plan = plan(&build_infantry_domain(), &moving).unwrap();
        assert_eq!(moving_plan.mtr, Mtr(vec![3, 0]));
        assert_eq!(
            moving_plan.steps[0].operator,
            BoundOperator::MoveTo {
                target: assigned_task.station
            }
        );

        let holding = PlannerState {
            assigned_task: Some(assigned_task),
            at_assigned_station: true,
            ..Default::default()
        };
        let holding_plan = plan(&build_infantry_domain(), &holding).unwrap();
        assert_eq!(holding_plan.mtr, Mtr(vec![3, 1]));
        assert_eq!(holding_plan.steps[0].operator, BoundOperator::Hold);
    }

    #[test]
    fn expired_task_is_not_executed() {
        let state = PlannerState {
            tick: 10,
            assigned_task: Some(AssignedTaskBelief {
                plan_id: CommandPlanId(3),
                issued_tick: 7,
                kind: AssignedTaskKind::HoldStation,
                station: PositionTarget::new(Vec2::X, None),
                fallback: PositionTarget::new(Vec2::ZERO, None),
                expires_at: Some(10),
            }),
            ..Default::default()
        };
        let planned = plan(&build_infantry_domain(), &state).unwrap();
        assert_eq!(planned.mtr, Mtr(vec![6]));
    }

    #[test]
    fn wounded_with_hostile_retreats_even_if_engage_possible() {
        let domain = build_infantry_domain();
        let mut state = state_with_hostile(ContactKind::Visual, 10);
        state.health_frac = 0.2;
        let plan = plan(&domain, &state).unwrap();

        assert_eq!(plan.mtr, Mtr(vec![0]));
        assert_eq!(plan.steps[0].task_name, "MoveAwayFromNearestHostile");
        assert_eq!(
            plan.steps[0].operator,
            BoundOperator::MoveTo {
                target: PositionTarget::new(Vec2::new(-30.0, 0.0), None)
            }
        );
    }
}
