use super::domain::{BoundOperator, Domain, DomainBuilder, Method, TaskId, always, bind_hold};
use super::soldier::{
    add_soldier_tasks, fresh_visual_hostile_with_ammo, has_active_assigned_task,
    low_health_with_hostile, stale_recent_hostile,
};
use super::state::{HoldStationAssignment, PlannerState};
use crate::gameplay::command_plans::{CommandPlanArea, CommandPlanKind};
use crate::gameplay::formations::{FormationKind, FormationSpec, generate_formation_positions};
use crate::gameplay::spatial::PositionTarget;
use bevy::prelude::*;

const FALLBACK_FORMATION_SPACING_M: f32 = 3.0;

pub struct LeadershipTasks {
    pub execute_plan: TaskId,
}

fn add_fallback_task(builder: &mut DomainBuilder) -> TaskId {
    let move_to_fallback = builder.primitive_with_reason(
        "MoveToFallbackPoint",
        "assigned plan or task expired; regroup at the preplanned rally point",
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
        "assigned Hold Line plan has an undelegated subordinate station",
        has_next_hold_station,
        bind_next_hold_station,
        mark_next_station_delegated_in_simulation,
    );
    let move_to_own_station = builder.primitive_with_reason(
        "MoveToOwnCommandPlanStation",
        "all other Hold Line stations are delegated; occupy own formation station",
        should_move_to_own_plan_station,
        bind_move_to_own_plan_station,
        effect_arrive_at_own_plan_station,
    );
    let hold_own_station = builder.primitive_with_reason(
        "HoldOwnCommandPlanStation",
        "own Hold Line formation station has been reached",
        at_own_active_plan_station,
        bind_hold,
        super::domain::no_effect,
    );
    let execute_plan = builder.compound(
        "ExecuteAssignedCommandPlan",
        vec![
            Method {
                name: "DelegateNextStation",
                preconditions: has_next_hold_station,
                subtasks: vec![delegate],
            },
            Method {
                name: "MoveToOwnStation",
                preconditions: should_move_to_own_plan_station,
                subtasks: vec![move_to_own_station],
            },
            Method {
                name: "HoldOwnStation",
                preconditions: at_own_active_plan_station,
                subtasks: vec![hold_own_station],
            },
        ],
    );

    LeadershipTasks { execute_plan }
}

/// One infantry repertoire shared by ordinary squad members and whoever
/// currently has command responsibility. Leadership methods are inert unless
/// planner state contains a valid assigned plan.
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
                name: "ExecuteAssignedCommandPlan",
                preconditions: active_hold_line_plan,
                subtasks: vec![leadership.execute_plan],
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
    fallback_active(state) && !state.at_fallback_target
}

fn at_active_fallback(state: &PlannerState) -> bool {
    fallback_active(state) && state.at_fallback_target
}

fn bind_move_to_fallback(state: &PlannerState) -> Option<BoundOperator> {
    Some(BoundOperator::MoveTo {
        target: state.fallback_target?,
    })
}

fn effect_arrive_at_fallback(state: &mut PlannerState) {
    let Some(target) = state.fallback_target else {
        return;
    };
    state.position_m = target.position_m;
    if let Some(heading) = target.heading_radians {
        state.heading_radians = heading;
    }
    state.at_fallback_target = true;
    state.has_move_target = true;
}

fn active_hold_line_plan(state: &PlannerState) -> bool {
    state.has_command_responsibility
        && state.assigned_plan.is_some_and(|plan| {
            plan.kind == CommandPlanKind::HoldLine
                && matches!(plan.area, CommandPlanArea::Line { .. })
                && !state.plan_is_expired()
        })
}

fn has_next_hold_station(state: &PlannerState) -> bool {
    active_hold_line_plan(state) && state.next_hold_station.is_some()
}

fn should_move_to_own_plan_station(state: &PlannerState) -> bool {
    active_hold_line_plan(state)
        && state.plan_delegation_complete
        && state.own_plan_target.is_some()
        && !state.at_own_plan_target
}

fn at_own_active_plan_station(state: &PlannerState) -> bool {
    active_hold_line_plan(state)
        && state.plan_delegation_complete
        && state.own_plan_target.is_some()
        && state.at_own_plan_target
}

fn bind_move_to_own_plan_station(state: &PlannerState) -> Option<BoundOperator> {
    Some(BoundOperator::MoveTo {
        target: state.own_plan_target?,
    })
}

fn effect_arrive_at_own_plan_station(state: &mut PlannerState) {
    let Some(target) = state.own_plan_target else {
        return;
    };
    state.position_m = target.position_m;
    if let Some(heading) = target.heading_radians {
        state.heading_radians = heading;
    }
    state.at_own_plan_target = true;
    state.has_move_target = true;
}

fn bind_next_hold_station(state: &PlannerState) -> Option<BoundOperator> {
    let plan = state.assigned_plan?;
    let assignment = state.next_hold_station?;
    Some(BoundOperator::DelegateHoldStation {
        plan_id: plan.id,
        plan_issued_tick: plan.issued_tick,
        assignee: assignment.assignee,
        station: assignment.station,
        fallback: assignment.fallback,
        expires_at: plan.expires_at,
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
/// current plan coordinator, to one evenly spaced line station. Participants
/// retain the caller's organizational order. Coordinator is a transient command
/// relationship, not a special unit archetype.
pub fn decompose_hold_line(
    from_m: Vec2,
    to_m: Vec2,
    rally_point_m: Vec2,
    coordinator: Entity,
    other_participants: &[Entity],
) -> Vec<HoldStationAssignment> {
    let mut others = Vec::with_capacity(other_participants.len());
    for &participant in other_participants {
        if participant != coordinator && !others.contains(&participant) {
            others.push(participant);
        }
    }

    let participant_count = others.len() + 1;
    let formation_heading = fallback_facing(from_m, to_m, rally_point_m);
    let formation_positions = generate_formation_positions(
        FormationSpec {
            kind: FormationKind::Wedge,
            anchor_m: rally_point_m,
            facing_radians: formation_heading,
            lateral_spacing_m: FALLBACK_FORMATION_SPACING_M,
            depth_spacing_m: FALLBACK_FORMATION_SPACING_M,
        },
        participant_count,
    )
    .expect("validated Hold Line geometry produces a valid fallback formation");
    let fallback_assignments: Vec<_> = std::iter::once(coordinator)
        .chain(others.iter().copied())
        .zip(formation_positions)
        .collect();

    let coordinator_slot = (participant_count - 1) / 2;
    let mut line_others = others.into_iter();
    (0..participant_count)
        .map(|index| {
            let assignee = if index == coordinator_slot {
                coordinator
            } else {
                line_others
                    .next()
                    .expect("one participant per non-command slot")
            };
            let station_position_m = if participant_count == 1 {
                (from_m + to_m) / 2.0
            } else {
                from_m.lerp(to_m, index as f32 / (participant_count - 1) as f32)
            };
            let fallback_position_m = fallback_assignments
                .iter()
                .find_map(|(member, station)| (*member == assignee).then_some(*station))
                .expect("every Hold Line participant has a fallback formation station");
            HoldStationAssignment {
                assignee,
                station: PositionTarget::new(station_position_m, Some(formation_heading)),
                fallback: PositionTarget::new(fallback_position_m, Some(formation_heading)),
            }
        })
        .collect()
}

fn fallback_facing(from_m: Vec2, to_m: Vec2, rally_point_m: Vec2) -> f32 {
    let toward_line = (from_m + to_m) / 2.0 - rally_point_m;
    if toward_line.length_squared() > f32::EPSILON {
        toward_line.to_angle()
    } else {
        (to_m - from_m).perp().to_angle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::htn::planner::{Mtr, plan};
    use crate::ai::htn::state::AssignedCommandPlanBelief;
    use crate::gameplay::command_plans::CommandPlanId;

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
            Vec2::new(0.0, -10.0),
            coordinator,
            &[c, a, b],
        );

        assert_eq!(
            assignments.iter().map(|a| a.assignee).collect::<Vec<_>>(),
            vec![c, coordinator, a, b]
        );
        for pair in assignments.windows(2) {
            assert!(
                (pair[1].station.position_m.x - pair[0].station.position_m.x - 20.0 / 3.0).abs()
                    < 0.001
            );
        }
        for (index, assignment) in assignments.iter().enumerate() {
            assert!(assignments[index + 1..].iter().all(|other| {
                assignment
                    .fallback
                    .position_m
                    .distance_squared(other.fallback.position_m)
                    > f32::EPSILON
            }));
        }
        assert_eq!(
            assignments
                .iter()
                .find(|assignment| assignment.assignee == coordinator)
                .unwrap()
                .fallback
                .position_m,
            Vec2::new(0.0, -10.0)
        );
        assert!(assignments.iter().all(|assignment| {
            assignment.station.heading_radians == Some(std::f32::consts::FRAC_PI_2)
                && assignment.fallback.heading_radians == Some(std::f32::consts::FRAC_PI_2)
        }));
    }

    #[test]
    fn lone_coordinator_receives_the_midpoint() {
        let coordinator = Entity::from_raw_u32(1).unwrap();
        let rally = Vec2::new(5.0, -10.0);
        let assignments =
            decompose_hold_line(Vec2::ZERO, Vec2::new(10.0, 0.0), rally, coordinator, &[]);
        assert_eq!(assignments[0].assignee, coordinator);
        assert_eq!(assignments[0].station.position_m, Vec2::new(5.0, 0.0));
        assert_eq!(assignments[0].fallback.position_m, rally);
        assert_eq!(
            assignments[0].station.heading_radians,
            assignments[0].fallback.heading_radians
        );
    }

    #[test]
    fn assigned_plan_outranks_investigation_and_idle() {
        let assignee = Entity::from_raw_u32(2).unwrap();
        let state = PlannerState {
            has_command_responsibility: true,
            assigned_plan: Some(AssignedCommandPlanBelief {
                id: CommandPlanId(1),
                issued_tick: 10,
                kind: CommandPlanKind::HoldLine,
                area: CommandPlanArea::Line {
                    from_m: Vec2::ZERO,
                    to_m: Vec2::X,
                },
                rally_point_m: Vec2::NEG_Y,
                expires_at: None,
            }),
            next_hold_station: Some(HoldStationAssignment {
                assignee,
                station: PositionTarget::new(Vec2::X, Some(0.0)),
                fallback: PositionTarget::new(Vec2::NEG_Y, Some(0.0)),
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
        let fallback_target = PositionTarget::new(rally_point, Some(1.0));
        let moving = PlannerState {
            fallback_target: Some(fallback_target),
            ..Default::default()
        };
        let planned = plan(&build_infantry_domain(), &moving).unwrap();
        assert_eq!(planned.mtr, Mtr(vec![2, 0]));
        assert_eq!(
            planned.steps[0].operator,
            BoundOperator::MoveTo {
                target: fallback_target
            }
        );

        let arrived = PlannerState {
            position_m: rally_point,
            heading_radians: 1.0,
            fallback_target: Some(fallback_target),
            at_fallback_target: true,
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
            assigned_plan: Some(AssignedCommandPlanBelief {
                id: CommandPlanId(1),
                issued_tick: 10,
                kind: CommandPlanKind::HoldLine,
                area: CommandPlanArea::Line {
                    from_m: Vec2::ZERO,
                    to_m: Vec2::X,
                },
                rally_point_m: Vec2::NEG_Y,
                expires_at: None,
            }),
            plan_delegation_complete: true,
            own_plan_target: Some(PositionTarget::new(Vec2::new(5.0, 0.0), Some(0.5))),
            ..Default::default()
        };
        let planned = plan(&build_infantry_domain(), &state).unwrap();

        assert_eq!(planned.mtr, Mtr(vec![4, 1]));
        assert_eq!(
            planned.steps[0].operator,
            BoundOperator::MoveTo {
                target: PositionTarget::new(Vec2::new(5.0, 0.0), Some(0.5))
            }
        );
    }
}
