use super::domain::{
    Domain, DomainBuilder, Method, always, bind_fire_at_nearest_hostile, bind_hold,
    bind_move_30m_away_from_nearest_hostile, bind_move_to_last_known_hostile_position,
};
use super::state::PlannerState;
use crate::ai::perception::ContactKind;

const LOW_HEALTH_FRAC: f32 = 0.35;
pub(crate) const FRESH_CONTACT_TICKS: u64 = 1;
const RECENT_CONTACT_TICKS: u64 = 100;
const MIN_ENGAGE_CONFIDENCE: f32 = 0.25;
const RETREAT_DISTANCE_M: f32 = 30.0;

pub fn build_soldier_domain() -> Domain {
    let mut builder = DomainBuilder::new();

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

    let root = builder.compound(
        "BeSoldier",
        vec![
            Method {
                name: "Survive",
                preconditions: low_health_with_hostile,
                subtasks: vec![retreat],
            },
            Method {
                name: "Engage",
                preconditions: fresh_visual_hostile_with_ammo,
                subtasks: vec![fire],
            },
            Method {
                name: "Investigate",
                preconditions: stale_recent_hostile,
                subtasks: vec![investigate],
            },
            Method {
                name: "Idle",
                preconditions: always,
                subtasks: vec![hold],
            },
        ],
    );

    builder.build(root)
}

fn hostile_present(state: &PlannerState) -> bool {
    state.nearest_hostile.is_some()
}

fn low_health_with_hostile(state: &PlannerState) -> bool {
    state.health_frac < LOW_HEALTH_FRAC && hostile_present(state)
}

fn fresh_visual_hostile_with_ammo(state: &PlannerState) -> bool {
    let Some(hostile) = state.nearest_hostile else {
        return false;
    };

    state.has_ammo
        && hostile.kind == ContactKind::Visual
        && hostile.confidence >= MIN_ENGAGE_CONFIDENCE
        && state.hostile_is_fresh(FRESH_CONTACT_TICKS)
}

fn stale_recent_hostile(state: &PlannerState) -> bool {
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
    use crate::ai::htn::planner::{Mtr, plan};
    use crate::ai::htn::state::HostileBelief;
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
        }
    }

    #[test]
    fn no_hostile_idles() {
        let domain = build_soldier_domain();
        let plan = plan(&domain, &PlannerState::default()).unwrap();

        assert_eq!(plan.mtr, Mtr(vec![3]));
        assert_eq!(plan.steps[0].task_name, "Hold");
    }

    #[test]
    fn fresh_visual_hostile_engages() {
        let domain = build_soldier_domain();
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
        let domain = build_soldier_domain();
        let state = state_with_hostile(ContactKind::Visual, 5);
        let plan = plan(&domain, &state).unwrap();

        assert_eq!(plan.mtr, Mtr(vec![2]));
        assert_eq!(plan.steps[0].task_name, "MoveToLastKnownHostilePosition");
        assert_eq!(
            plan.steps[0].operator,
            BoundOperator::MoveTo {
                destination_m: Vec2::new(10.0, 0.0)
            }
        );
    }

    #[test]
    fn wounded_with_hostile_retreats_even_if_engage_possible() {
        let domain = build_soldier_domain();
        let mut state = state_with_hostile(ContactKind::Visual, 10);
        state.health_frac = 0.2;
        let plan = plan(&domain, &state).unwrap();

        assert_eq!(plan.mtr, Mtr(vec![0]));
        assert_eq!(plan.steps[0].task_name, "MoveAwayFromNearestHostile");
        assert_eq!(
            plan.steps[0].operator,
            BoundOperator::MoveTo {
                destination_m: Vec2::new(-30.0, 0.0)
            }
        );
    }
}
