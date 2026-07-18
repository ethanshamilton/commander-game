use super::domain::{BoundOperator, Domain, DomainBuilder, Method, TaskId, always, bind_hold};
use super::soldier::{
    add_soldier_tasks, fresh_visual_hostile_with_ammo, has_active_assigned_task,
    low_health_with_hostile, stale_recent_hostile,
};
use super::state::{HoldStationAssignment, PlannerState};
use crate::gameplay::missions::{MissionArea, MissionKind};
use bevy::prelude::*;

pub struct LeadershipTasks {
    pub execute_mission: TaskId,
}

fn add_fallback_task(builder: &mut DomainBuilder) -> TaskId {
    let move_to_fallback = builder.primitive_with_reason(
        "MoveToFallbackPoint",
        "assigned mission or task expired; regroup at the preplanned rally point",
        fallback_needs_movement,
        bind_move_to_fallback,
        effect_arrive_at_fallback,
    );
    let hold_at_fallback = builder.primitive_with_reason(
        "HoldAtFallbackPoint",
        "rally point reached after assignment expiry",
        at_active_fallback,
        bind_hold,
        super::domain::no_effect,
    );
    builder.compound(
        "ExecuteFallback",
        vec![
            Method {
                name: "MoveToFallback",
                preconditions: fallback_needs_movement,
                subtasks: vec![move_to_fallback],
            },
            Method {
                name: "HoldAtFallback",
                preconditions: at_active_fallback,
                subtasks: vec![hold_at_fallback],
            },
        ],
    )
}

pub fn add_leadership_tasks(builder: &mut DomainBuilder) -> LeadershipTasks {
    let delegate = builder.primitive_with_reason(
        "DelegateHoldStation",
        "assigned Hold Line mission has an undelegated subordinate station",
        has_next_hold_station,
        bind_next_hold_station,
        mark_next_station_delegated_in_simulation,
    );
    let move_to_own_station = builder.primitive_with_reason(
        "MoveToOwnMissionStation",
        "all other Hold Line stations are delegated; occupy own formation station",
        should_move_to_own_mission_station,
        bind_move_to_own_mission_station,
        effect_arrive_at_own_mission_station,
    );
    let hold_own_station = builder.primitive_with_reason(
        "HoldOwnMissionStation",
        "own Hold Line formation station has been reached",
        at_own_active_mission_station,
        bind_hold,
        super::domain::no_effect,
    );
    let execute_mission = builder.compound(
        "ExecuteAssignedMission",
        vec![
            Method {
                name: "DelegateNextStation",
                preconditions: has_next_hold_station,
                subtasks: vec![delegate],
            },
            Method {
                name: "MoveToOwnStation",
                preconditions: should_move_to_own_mission_station,
                subtasks: vec![move_to_own_station],
            },
            Method {
                name: "HoldOwnStation",
                preconditions: at_own_active_mission_station,
                subtasks: vec![hold_own_station],
            },
        ],
    );

    LeadershipTasks { execute_mission }
}

/// One infantry repertoire shared by ordinary squad members and whoever
/// currently has command responsibility. Leadership methods are inert unless
/// planner state contains a valid assigned mission.
pub fn build_infantry_domain() -> Domain {
    let mut builder = DomainBuilder::new();
    let soldier = add_soldier_tasks(&mut builder);
    let fallback = add_fallback_task(&mut builder);
    let leadership = add_leadership_tasks(&mut builder);

    let root = builder.compound(
        "BeInfantry",
        vec![
            Method {
                name: "Survive",
                preconditions: low_health_with_hostile,
                subtasks: vec![soldier.retreat],
            },
            Method {
                name: "Engage",
                preconditions: fresh_visual_hostile_with_ammo,
                subtasks: vec![soldier.fire],
            },
            Method {
                name: "ExecuteFallback",
                preconditions: fallback_active,
                subtasks: vec![fallback],
            },
            Method {
                name: "ExecuteAssignedTask",
                preconditions: has_active_assigned_task,
                subtasks: vec![soldier.execute_assigned_task],
            },
            Method {
                name: "ExecuteAssignedMission",
                preconditions: active_hold_line_mission,
                subtasks: vec![leadership.execute_mission],
            },
            Method {
                name: "Investigate",
                preconditions: stale_recent_hostile,
                subtasks: vec![soldier.investigate],
            },
            Method {
                name: "Idle",
                preconditions: always,
                subtasks: vec![soldier.hold],
            },
        ],
    );

    builder.build(root)
}

fn fallback_active(state: &PlannerState) -> bool {
    state.fallback_is_active()
}

fn fallback_needs_movement(state: &PlannerState) -> bool {
    fallback_active(state) && !state.at_fallback_point
}

fn at_active_fallback(state: &PlannerState) -> bool {
    fallback_active(state) && state.at_fallback_point
}

fn bind_move_to_fallback(state: &PlannerState) -> Option<BoundOperator> {
    Some(BoundOperator::MoveTo {
        destination_m: state.fallback_point_m?,
    })
}

fn effect_arrive_at_fallback(state: &mut PlannerState) {
    let Some(fallback_point_m) = state.fallback_point_m else {
        return;
    };
    state.position_m = fallback_point_m;
    state.at_fallback_point = true;
    state.has_move_target = true;
}

fn active_hold_line_mission(state: &PlannerState) -> bool {
    state.has_command_responsibility
        && state.assigned_mission.is_some_and(|mission| {
            mission.kind == MissionKind::HoldLine
                && matches!(mission.area, MissionArea::Line { .. })
                && !state.mission_is_expired()
        })
}

fn has_next_hold_station(state: &PlannerState) -> bool {
    active_hold_line_mission(state) && state.next_hold_station.is_some()
}

fn should_move_to_own_mission_station(state: &PlannerState) -> bool {
    active_hold_line_mission(state)
        && state.mission_delegation_complete
        && state.own_mission_station_m.is_some()
        && !state.at_own_mission_station
}

fn at_own_active_mission_station(state: &PlannerState) -> bool {
    active_hold_line_mission(state)
        && state.mission_delegation_complete
        && state.own_mission_station_m.is_some()
        && state.at_own_mission_station
}

fn bind_move_to_own_mission_station(state: &PlannerState) -> Option<BoundOperator> {
    Some(BoundOperator::MoveTo {
        destination_m: state.own_mission_station_m?,
    })
}

fn effect_arrive_at_own_mission_station(state: &mut PlannerState) {
    let Some(station_m) = state.own_mission_station_m else {
        return;
    };
    state.position_m = station_m;
    state.at_own_mission_station = true;
    state.has_move_target = true;
}

fn bind_next_hold_station(state: &PlannerState) -> Option<BoundOperator> {
    let mission = state.assigned_mission?;
    let assignment = state.next_hold_station?;
    Some(BoundOperator::DelegateHoldStation {
        mission_id: mission.id,
        mission_issued_tick: mission.issued_tick,
        assignee: assignment.assignee,
        station_m: assignment.station_m,
        rally_point_m: mission.rally_point_m,
        expires_at: mission.expires_at,
    })
}

fn mark_next_station_delegated_in_simulation(state: &mut PlannerState) {
    let Some(next) = state.next_hold_station.take() else {
        return;
    };
    if !state.delegated_assignees.contains(&next.assignee) {
        state.delegated_assignees.push(next.assignee);
    }
}

/// Deterministically assign every living formation participant, including the
/// current mission coordinator, to one evenly spaced line station. Coordinator
/// is a transient command relationship, not a special unit archetype.
pub fn decompose_hold_line(
    from_m: Vec2,
    to_m: Vec2,
    coordinator: Entity,
    other_participants: &[Entity],
) -> Vec<HoldStationAssignment> {
    let mut others = other_participants.to_vec();
    others.sort_by_key(|entity| entity.index());
    others.dedup();
    others.retain(|entity| *entity != coordinator);

    let participant_count = others.len() + 1;
    if participant_count == 1 {
        return vec![HoldStationAssignment {
            assignee: coordinator,
            station_m: (from_m + to_m) / 2.0,
        }];
    }

    let coordinator_slot = (participant_count - 1) / 2;
    let mut others = others.into_iter();
    (0..participant_count)
        .map(|index| HoldStationAssignment {
            assignee: if index == coordinator_slot {
                coordinator
            } else {
                others.next().expect("one participant per non-command slot")
            },
            station_m: from_m.lerp(to_m, index as f32 / (participant_count - 1) as f32),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::htn::planner::{Mtr, plan};
    use crate::ai::htn::state::AssignedMissionBelief;
    use crate::gameplay::missions::MissionId;

    #[test]
    fn decomposition_is_stable_and_evenly_spaced() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let c = world.spawn_empty().id();
        let coordinator = world.spawn_empty().id();
        let assignments = decompose_hold_line(
            Vec2::new(-10.0, 0.0),
            Vec2::new(10.0, 0.0),
            coordinator,
            &[c, a, b],
        );

        assert_eq!(
            assignments.iter().map(|a| a.assignee).collect::<Vec<_>>(),
            vec![a, coordinator, b, c]
        );
        for pair in assignments.windows(2) {
            assert!((pair[1].station_m.x - pair[0].station_m.x - 20.0 / 3.0).abs() < 0.001);
        }
    }

    #[test]
    fn lone_coordinator_receives_the_midpoint() {
        let coordinator = Entity::from_raw_u32(1).unwrap();
        let assignments = decompose_hold_line(Vec2::ZERO, Vec2::new(10.0, 0.0), coordinator, &[]);
        assert_eq!(assignments[0].assignee, coordinator);
        assert_eq!(assignments[0].station_m, Vec2::new(5.0, 0.0));
    }

    #[test]
    fn assigned_mission_outranks_investigation_and_idle() {
        let assignee = Entity::from_raw_u32(2).unwrap();
        let state = PlannerState {
            has_command_responsibility: true,
            assigned_mission: Some(AssignedMissionBelief {
                id: MissionId(1),
                issued_tick: 10,
                kind: MissionKind::HoldLine,
                area: MissionArea::Line {
                    from_m: Vec2::ZERO,
                    to_m: Vec2::X,
                },
                rally_point_m: Vec2::NEG_Y,
                expires_at: None,
            }),
            next_hold_station: Some(HoldStationAssignment {
                assignee,
                station_m: Vec2::X,
            }),
            ..Default::default()
        };
        let planned = plan(&build_infantry_domain(), &state).unwrap();

        assert_eq!(planned.mtr, Mtr(vec![4, 0]));
        assert!(matches!(
            planned.steps[0].operator,
            BoundOperator::DelegateHoldStation { assignee: target, .. } if target == assignee
        ));
    }

    #[test]
    fn expired_assignment_moves_to_fallback_then_holds() {
        let rally_point = Vec2::new(-5.0, 2.0);
        let moving = PlannerState {
            fallback_point_m: Some(rally_point),
            ..Default::default()
        };
        let planned = plan(&build_infantry_domain(), &moving).unwrap();
        assert_eq!(planned.mtr, Mtr(vec![2, 0]));
        assert_eq!(
            planned.steps[0].operator,
            BoundOperator::MoveTo {
                destination_m: rally_point
            }
        );

        let arrived = PlannerState {
            position_m: rally_point,
            fallback_point_m: Some(rally_point),
            at_fallback_point: true,
            ..Default::default()
        };
        let planned = plan(&build_infantry_domain(), &arrived).unwrap();
        assert_eq!(planned.mtr, Mtr(vec![2, 1]));
        assert_eq!(planned.steps[0].operator, BoundOperator::Hold);
    }

    #[test]
    fn coordinator_moves_to_own_station_after_delegation() {
        let state = PlannerState {
            has_command_responsibility: true,
            assigned_mission: Some(AssignedMissionBelief {
                id: MissionId(1),
                issued_tick: 10,
                kind: MissionKind::HoldLine,
                area: MissionArea::Line {
                    from_m: Vec2::ZERO,
                    to_m: Vec2::X,
                },
                rally_point_m: Vec2::NEG_Y,
                expires_at: None,
            }),
            mission_delegation_complete: true,
            own_mission_station_m: Some(Vec2::new(5.0, 0.0)),
            ..Default::default()
        };
        let planned = plan(&build_infantry_domain(), &state).unwrap();

        assert_eq!(planned.mtr, Mtr(vec![4, 1]));
        assert_eq!(
            planned.steps[0].operator,
            BoundOperator::MoveTo {
                destination_m: Vec2::new(5.0, 0.0)
            }
        );
    }
}
