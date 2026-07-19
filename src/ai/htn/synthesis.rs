use super::executor::{Autonomous, RecentResolvedShots};
use super::leader::decompose_hold_line;
use super::state::{
    AssignedCommandPlanBelief, AssignedTaskBelief, AssignedTaskKind, HostileBelief, PlannerState,
};
use crate::actors::units::{Alive, Health, Inventory, Soldier};
use crate::ai::perception::{AuditorySensor, Contact, ContactKind, ContactType, PerceptionMemory};
use crate::gameplay::combat::ResolvedShot;
use crate::gameplay::command::CommandForest;
use crate::gameplay::command_plans::{
    AssignedCommandPlan, AssignedTask, CommandPlanArea, CommandPlanDelegationProgress,
    CommandPlanKind, TaskDirective,
};
use crate::gameplay::simulation::{MovementOrder, SimulationClock};
use crate::gameplay::spatial::{BattlefieldPosition, Heading, PositionTarget};
use crate::intel::ReportedLifeStatus;
use bevy::prelude::*;
use std::cmp::Ordering;

/// Placeholder for richer threat memory. For now, recent nearby hostile contact
/// is a coarse suspicion signal; direct incoming-fire detection uses resolved
/// shot impacts supplied to synthesis.
const UNDER_FIRE_CONTACT_MAX_STALENESS_TICKS: u64 = 20;
const UNDER_FIRE_CONTACT_DISTANCE_M: f32 = 80.0;
pub const STATION_ARRIVAL_EPSILON_M: f32 = 0.25;
pub const HEADING_ARRIVAL_EPSILON_RADIANS: f32 = 0.05;

/// Per-unit planning snapshot, refreshed once per tick before Thinking systems run.
/// This is the unit's belief state — the single input to deliberation, step
/// dispatch, and step polling. Debug/trace views should read this, not raw memory.
#[derive(Component, Debug, Clone, Default)]
pub struct PlannerBelief {
    pub state: PlannerState,
}

pub fn synthesize_beliefs(
    clock: Res<SimulationClock>,
    recent_shots: Res<RecentResolvedShots>,
    command_forest: Option<Res<CommandForest>>,
    living_soldiers: Query<(), (With<Soldier>, With<Alive>)>,
    mut units: Query<
        (
            Entity,
            &BattlefieldPosition,
            Option<&Heading>,
            &Health,
            &Inventory,
            &PerceptionMemory,
            Option<&AuditorySensor>,
            Option<&MovementOrder>,
            Option<&AssignedCommandPlan>,
            Option<&AssignedTask>,
            Option<&CommandPlanDelegationProgress>,
            &mut PlannerBelief,
        ),
        (With<Soldier>, With<Alive>, With<Autonomous>),
    >,
) {
    for (
        entity,
        position,
        heading,
        health,
        inventory,
        memory,
        auditory,
        order,
        assigned_plan,
        assigned_task,
        delegation_progress,
        mut belief,
    ) in &mut units
    {
        let heading = heading.copied().unwrap_or(Heading(0.0));
        let mut state = synthesize_planner_state_with_heading(
            &clock,
            position,
            &heading,
            health,
            inventory,
            memory,
            auditory,
            order,
            &recent_shots.shots,
        );
        project_task_belief(&mut state, assigned_task);
        if let Some(command_forest) = command_forest.as_deref() {
            project_plan_belief(
                &mut state,
                entity,
                assigned_plan,
                delegation_progress,
                command_forest,
                &living_soldiers,
            );
        }
        project_expiry_fallback(&mut state);
        belief.state = state;
    }
}

fn project_task_belief(state: &mut PlannerState, assigned: Option<&AssignedTask>) {
    let Some(assigned) = assigned else {
        return;
    };

    match assigned.directive {
        TaskDirective::HoldStation {
            plan_id,
            station,
            fallback,
            expires_at,
        } => {
            state.assigned_task = Some(AssignedTaskBelief {
                plan_id,
                issued_tick: assigned.issued_tick,
                kind: AssignedTaskKind::HoldStation,
                station,
                fallback,
                expires_at,
            });
            state.at_assigned_station = target_is_reached(state, station);
        }
    }
}

fn project_expiry_fallback(state: &mut PlannerState) {
    // CommandPlan and task assignments live in separate ECS lanes. If both remain
    // installed, the newest directive is the unit's current source of intent.
    let fallback = match (state.assigned_plan, state.assigned_task) {
        (Some(plan), Some(task)) if task.issued_tick > plan.issued_tick => {
            task.expires_at.map(|expiry| (expiry, task.fallback))
        }
        (Some(plan), _) => plan.expires_at.zip(state.own_fallback_target),
        (None, Some(task)) => task.expires_at.map(|expiry| (expiry, task.fallback)),
        (None, None) => None,
    };

    let Some((expires_at, fallback_target)) = fallback else {
        return;
    };
    if state.tick < expires_at {
        return;
    }

    state.fallback_target = Some(fallback_target);
    state.at_fallback_target = target_is_reached(state, fallback_target);
}

fn project_plan_belief(
    state: &mut PlannerState,
    entity: Entity,
    assigned: Option<&AssignedCommandPlan>,
    progress: Option<&CommandPlanDelegationProgress>,
    command_forest: &CommandForest,
    living_soldiers: &Query<(), (With<Soldier>, With<Alive>)>,
) {
    let Some(assigned) = assigned else {
        return;
    };

    let plan = AssignedCommandPlanBelief {
        id: assigned.plan.id,
        issued_tick: assigned.issued_tick,
        kind: assigned.plan.kind,
        area: assigned.plan.area,
        rally_point_m: assigned.plan.rally_point_m,
        expires_at: assigned.plan.expires_at,
    };
    state.assigned_plan = Some(plan);
    state.has_command_responsibility = true;

    if plan.kind != CommandPlanKind::HoldLine {
        return;
    }
    let CommandPlanArea::Line { from_m, to_m } = plan.area else {
        return;
    };

    let subordinates: Vec<_> = command_forest
        .subordinates_of(entity)
        .iter()
        .copied()
        .filter(|subordinate| living_soldiers.get(*subordinate).is_ok())
        .collect();
    let assignments = decompose_hold_line(from_m, to_m, plan.rally_point_m, entity, &subordinates);
    let delegated = progress
        .filter(|progress| progress.plan == Some(plan.identity()))
        .map(|progress| progress.delegated_to.as_slice())
        .unwrap_or(&[]);

    state.delegated_assignees = delegated.to_vec();
    let own_assignment = assignments
        .iter()
        .find(|assignment| assignment.assignee == entity);
    state.own_plan_target = own_assignment.map(|assignment| assignment.station);
    state.own_fallback_target = own_assignment.map(|assignment| assignment.fallback);
    state.at_own_plan_target = state
        .own_plan_target
        .is_some_and(|target| target_is_reached(state, target));

    if state.plan_is_expired() {
        return;
    }

    state.next_hold_station = assignments.iter().copied().find(|assignment| {
        assignment.assignee != entity && !delegated.contains(&assignment.assignee)
    });
    state.plan_delegation_complete = state.next_hold_station.is_none();
}

fn target_is_reached(state: &PlannerState, target: PositionTarget) -> bool {
    target.is_reached(
        state.position_m,
        state.heading_radians,
        STATION_ARRIVAL_EPSILON_M,
        HEADING_ARRIVAL_EPSILON_RADIANS,
    )
}

pub fn synthesize_planner_state(
    clock: &SimulationClock,
    position: &BattlefieldPosition,
    health: &Health,
    inventory: &Inventory,
    memory: &PerceptionMemory,
    auditory_sensor: Option<&AuditorySensor>,
    current_order: Option<&MovementOrder>,
    recent_shots: &[ResolvedShot],
) -> PlannerState {
    synthesize_planner_state_with_heading(
        clock,
        position,
        &Heading(0.0),
        health,
        inventory,
        memory,
        auditory_sensor,
        current_order,
        recent_shots,
    )
}

fn synthesize_planner_state_with_heading(
    clock: &SimulationClock,
    position: &BattlefieldPosition,
    heading: &Heading,
    health: &Health,
    inventory: &Inventory,
    memory: &PerceptionMemory,
    auditory_sensor: Option<&AuditorySensor>,
    current_order: Option<&MovementOrder>,
    recent_shots: &[ResolvedShot],
) -> PlannerState {
    let nearest_hostile = nearest_hostile_belief(position.0, memory);
    let under_fire = under_fire_from_impacts(position.0, auditory_sensor, recent_shots)
        || under_fire_from_contacts(clock.tick, position.0, memory);

    PlannerState {
        position_m: position.0,
        heading_radians: heading.0,
        health_frac: health_fraction(health),
        has_ammo: inventory.has_ammo(),
        nearest_hostile,
        under_fire,
        has_move_target: matches!(current_order, Some(MovementOrder::MoveTo { .. })),
        tick: clock.tick,
        ..Default::default()
    }
}

fn health_fraction(health: &Health) -> f32 {
    if health.max <= 0 {
        return 0.0;
    }

    (health.current as f32 / health.max as f32).clamp(0.0, 1.0)
}

fn nearest_hostile_belief(position_m: Vec2, memory: &PerceptionMemory) -> Option<HostileBelief> {
    let mut best_per_target: Vec<&Contact> = Vec::new();

    for contact in memory.contacts.iter().filter(|contact| {
        contact.contact_type == ContactType::Hostile
            && contact.observed_life_status == ReportedLifeStatus::Alive
            && contact.confidence > 0.0
    }) {
        if let Some(existing) = best_per_target
            .iter_mut()
            .find(|existing| existing.target == contact.target)
        {
            if contact_observation_cmp(contact, existing).is_gt() {
                *existing = contact;
            }
        } else {
            best_per_target.push(contact);
        }
    }

    best_per_target
        .into_iter()
        .min_by(|a, b| {
            let a_distance = position_m.distance_squared(a.last_seen_position_m);
            let b_distance = position_m.distance_squared(b.last_seen_position_m);
            a_distance.total_cmp(&b_distance)
        })
        .map(|contact| HostileBelief {
            entity: contact.target,
            position_m: contact.last_seen_position_m,
            confidence: contact.confidence,
            last_seen_tick: contact.last_seen_tick,
            kind: contact.kind,
        })
}

fn contact_observation_cmp(a: &Contact, b: &Contact) -> Ordering {
    a.last_seen_tick
        .cmp(&b.last_seen_tick)
        .then_with(|| a.confidence.total_cmp(&b.confidence))
        .then_with(|| contact_kind_priority(a.kind).cmp(&contact_kind_priority(b.kind)))
}

fn contact_kind_priority(kind: ContactKind) -> u8 {
    match kind {
        ContactKind::Visual => 4,
        ContactKind::Auditory => 3,
        ContactKind::Radar => 2,
        ContactKind::Unknown => 1,
    }
}

fn under_fire_from_impacts(
    position_m: Vec2,
    auditory_sensor: Option<&AuditorySensor>,
    recent_shots: &[ResolvedShot],
) -> bool {
    let Some(auditory_sensor) = auditory_sensor else {
        return false;
    };

    let range_sq = auditory_sensor.range_m * auditory_sensor.range_m;
    recent_shots
        .iter()
        .any(|shot| position_m.distance_squared(shot.impact_position_m) <= range_sq)
}

fn under_fire_from_contacts(tick: u64, position_m: Vec2, memory: &PerceptionMemory) -> bool {
    memory.contacts.iter().any(|contact| {
        contact.contact_type == ContactType::Hostile
            && contact.observed_life_status == ReportedLifeStatus::Alive
            && tick.saturating_sub(contact.last_seen_tick) <= UNDER_FIRE_CONTACT_MAX_STALENESS_TICKS
            && position_m.distance_squared(contact.last_seen_position_m)
                <= UNDER_FIRE_CONTACT_DISTANCE_M * UNDER_FIRE_CONTACT_DISTANCE_M
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::units::{Item, ItemKind};
    use bevy::ecs::world::World;

    fn inventory_with_ammo(count: u32) -> Inventory {
        Inventory {
            items: vec![Item {
                kind: ItemKind::Ammo,
                count,
            }],
        }
    }

    fn contact(
        target: Entity,
        position_m: Vec2,
        tick: u64,
        confidence: f32,
        kind: ContactKind,
        contact_type: ContactType,
        life: ReportedLifeStatus,
    ) -> Contact {
        Contact {
            target,
            last_seen_position_m: position_m,
            last_seen_time_s: 0.0,
            last_seen_tick: tick,
            confidence,
            observed_life_status: life,
            kind,
            contact_type,
        }
    }

    fn shot_at(impact_position_m: Vec2) -> ResolvedShot {
        let mut world = World::new();
        let shooter = world.spawn_empty().id();
        let target = world.spawn_empty().id();
        ResolvedShot {
            shooter,
            target,
            shooter_position_m: Vec2::ZERO,
            target_position_m: Vec2::ZERO,
            impact_position_m,
            hit: false,
            damage: 0,
            projectile_speed_mps: 700.0,
            tracer_length_m: 8.0,
        }
    }

    #[test]
    fn health_fraction_is_clamped() {
        assert_eq!(
            health_fraction(&Health {
                current: 150,
                max: 100
            }),
            1.0
        );
        assert_eq!(
            health_fraction(&Health {
                current: -10,
                max: 100
            }),
            0.0
        );
        assert_eq!(
            health_fraction(&Health {
                current: 10,
                max: 0
            }),
            0.0
        );
    }

    #[test]
    fn ammo_maps_from_inventory() {
        let clock = SimulationClock::default();
        let position = BattlefieldPosition(Vec2::ZERO);
        let memory = PerceptionMemory::default();
        let health = Health {
            current: 100,
            max: 100,
        };

        let empty = synthesize_planner_state(
            &clock,
            &position,
            &health,
            &inventory_with_ammo(0),
            &memory,
            None,
            None,
            &[],
        );
        let loaded = synthesize_planner_state(
            &clock,
            &position,
            &health,
            &inventory_with_ammo(1),
            &memory,
            None,
            None,
            &[],
        );

        assert!(!empty.has_ammo);
        assert!(loaded.has_ammo);
    }

    #[test]
    fn nearest_hostile_ignores_friendlies_dead_and_zero_confidence() {
        let mut world = World::new();
        let friendly = world.spawn_empty().id();
        let dead_hostile = world.spawn_empty().id();
        let zero_confidence = world.spawn_empty().id();
        let alive_hostile = world.spawn_empty().id();
        let memory = PerceptionMemory {
            contacts: vec![
                contact(
                    friendly,
                    Vec2::new(1.0, 0.0),
                    1,
                    1.0,
                    ContactKind::Visual,
                    ContactType::Friendly,
                    ReportedLifeStatus::Alive,
                ),
                contact(
                    dead_hostile,
                    Vec2::new(2.0, 0.0),
                    1,
                    1.0,
                    ContactKind::Visual,
                    ContactType::Hostile,
                    ReportedLifeStatus::Dead,
                ),
                contact(
                    zero_confidence,
                    Vec2::new(3.0, 0.0),
                    1,
                    0.0,
                    ContactKind::Visual,
                    ContactType::Hostile,
                    ReportedLifeStatus::Alive,
                ),
                contact(
                    alive_hostile,
                    Vec2::new(4.0, 0.0),
                    1,
                    1.0,
                    ContactKind::Visual,
                    ContactType::Hostile,
                    ReportedLifeStatus::Alive,
                ),
            ],
        };

        let belief = nearest_hostile_belief(Vec2::ZERO, &memory).unwrap();

        assert_eq!(belief.entity, alive_hostile);
    }

    #[test]
    fn nearest_hostile_chooses_nearest_after_deduping_targets() {
        let mut world = World::new();
        let far = world.spawn_empty().id();
        let near = world.spawn_empty().id();
        let memory = PerceptionMemory {
            contacts: vec![
                contact(
                    far,
                    Vec2::new(20.0, 0.0),
                    1,
                    1.0,
                    ContactKind::Visual,
                    ContactType::Hostile,
                    ReportedLifeStatus::Alive,
                ),
                contact(
                    near,
                    Vec2::new(5.0, 0.0),
                    1,
                    1.0,
                    ContactKind::Visual,
                    ContactType::Hostile,
                    ReportedLifeStatus::Alive,
                ),
            ],
        };

        let belief = nearest_hostile_belief(Vec2::ZERO, &memory).unwrap();

        assert_eq!(belief.entity, near);
    }

    #[test]
    fn newer_contact_wins_for_same_target() {
        let mut world = World::new();
        let target = world.spawn_empty().id();
        let memory = PerceptionMemory {
            contacts: vec![
                contact(
                    target,
                    Vec2::new(100.0, 0.0),
                    1,
                    1.0,
                    ContactKind::Visual,
                    ContactType::Hostile,
                    ReportedLifeStatus::Alive,
                ),
                contact(
                    target,
                    Vec2::new(10.0, 0.0),
                    2,
                    0.5,
                    ContactKind::Auditory,
                    ContactType::Hostile,
                    ReportedLifeStatus::Alive,
                ),
            ],
        };

        let belief = nearest_hostile_belief(Vec2::ZERO, &memory).unwrap();

        assert_eq!(belief.position_m, Vec2::new(10.0, 0.0));
        assert_eq!(belief.kind, ContactKind::Auditory);
    }

    #[test]
    fn visual_wins_contact_tie_over_auditory() {
        let mut world = World::new();
        let target = world.spawn_empty().id();
        let memory = PerceptionMemory {
            contacts: vec![
                contact(
                    target,
                    Vec2::new(20.0, 0.0),
                    1,
                    0.8,
                    ContactKind::Auditory,
                    ContactType::Hostile,
                    ReportedLifeStatus::Alive,
                ),
                contact(
                    target,
                    Vec2::new(10.0, 0.0),
                    1,
                    0.8,
                    ContactKind::Visual,
                    ContactType::Hostile,
                    ReportedLifeStatus::Alive,
                ),
            ],
        };

        let belief = nearest_hostile_belief(Vec2::ZERO, &memory).unwrap();

        assert_eq!(belief.position_m, Vec2::new(10.0, 0.0));
        assert_eq!(belief.kind, ContactKind::Visual);
    }

    #[test]
    fn under_fire_from_shot_impacts_in_auditory_range() {
        let sensor = AuditorySensor { range_m: 40.0 };

        assert!(under_fire_from_impacts(
            Vec2::ZERO,
            Some(&sensor),
            &[shot_at(Vec2::new(30.0, 0.0))],
        ));
        assert!(!under_fire_from_impacts(
            Vec2::ZERO,
            Some(&sensor),
            &[shot_at(Vec2::new(50.0, 0.0))],
        ));
        assert!(!under_fire_from_impacts(
            Vec2::ZERO,
            None,
            &[shot_at(Vec2::new(1.0, 0.0))],
        ));
    }

    #[test]
    fn under_fire_from_fresh_near_hostile_contact_fallback() {
        let mut world = World::new();
        let target = world.spawn_empty().id();
        let fresh_near = PerceptionMemory {
            contacts: vec![contact(
                target,
                Vec2::new(10.0, 0.0),
                90,
                1.0,
                ContactKind::Visual,
                ContactType::Hostile,
                ReportedLifeStatus::Alive,
            )],
        };
        let stale = PerceptionMemory {
            contacts: vec![contact(
                target,
                Vec2::new(10.0, 0.0),
                1,
                1.0,
                ContactKind::Visual,
                ContactType::Hostile,
                ReportedLifeStatus::Alive,
            )],
        };
        let far = PerceptionMemory {
            contacts: vec![contact(
                target,
                Vec2::new(100.0, 0.0),
                90,
                1.0,
                ContactKind::Visual,
                ContactType::Hostile,
                ReportedLifeStatus::Alive,
            )],
        };

        assert!(under_fire_from_contacts(100, Vec2::ZERO, &fresh_near));
        assert!(!under_fire_from_contacts(100, Vec2::ZERO, &stale));
        assert!(!under_fire_from_contacts(100, Vec2::ZERO, &far));
    }

    #[test]
    fn assigned_hold_station_projects_distance_and_expiry_facts() {
        let assigned = AssignedTask {
            directive: TaskDirective::HoldStation {
                plan_id: crate::gameplay::command_plans::CommandPlanId(4),
                station: PositionTarget::new(Vec2::new(5.0, 0.0), Some(0.0)),
                fallback: PositionTarget::new(Vec2::new(-5.0, 0.0), Some(1.0)),
                expires_at: Some(12),
            },
            assigned_by: Entity::PLACEHOLDER,
            issued_tick: 9,
            received_tick: 9,
        };
        let mut state = PlannerState {
            position_m: Vec2::new(5.1, 0.0),
            tick: 10,
            ..Default::default()
        };

        project_task_belief(&mut state, Some(&assigned));

        assert!(state.at_assigned_station);
        assert!(!state.assigned_task_is_expired());
        assert_eq!(state.assigned_task.unwrap().issued_tick, 9);

        state.heading_radians = 0.5;
        project_task_belief(&mut state, Some(&assigned));
        assert!(!state.at_assigned_station);
        state.heading_radians = 0.0;
        project_task_belief(&mut state, Some(&assigned));
        assert!(state.at_assigned_station);

        project_expiry_fallback(&mut state);
        assert_eq!(state.fallback_target, None);

        state.tick = 12;
        assert!(state.assigned_task_is_expired());
        project_expiry_fallback(&mut state);
        assert_eq!(
            state.fallback_target,
            Some(PositionTarget::new(Vec2::new(-5.0, 0.0), Some(1.0)))
        );
        assert!(!state.at_fallback_target);
    }

    #[test]
    fn newer_assignment_controls_expiry_fallback() {
        let mut state = PlannerState {
            position_m: Vec2::new(10.0, 0.0),
            tick: 20,
            assigned_plan: Some(AssignedCommandPlanBelief {
                id: crate::gameplay::command_plans::CommandPlanId(1),
                issued_tick: 10,
                kind: CommandPlanKind::HoldLine,
                area: CommandPlanArea::Line {
                    from_m: Vec2::ZERO,
                    to_m: Vec2::X,
                },
                rally_point_m: Vec2::NEG_X,
                expires_at: Some(15),
            }),
            assigned_task: Some(AssignedTaskBelief {
                plan_id: crate::gameplay::command_plans::CommandPlanId(2),
                issued_tick: 11,
                kind: AssignedTaskKind::HoldStation,
                station: PositionTarget::new(Vec2::X, None),
                fallback: PositionTarget::new(Vec2::new(10.0, 0.0), None),
                expires_at: Some(20),
            }),
            ..Default::default()
        };

        project_expiry_fallback(&mut state);

        assert_eq!(
            state.fallback_target,
            Some(PositionTarget::new(Vec2::new(10.0, 0.0), None))
        );
        assert!(state.at_fallback_target);
    }

    #[test]
    fn move_target_comes_from_current_order() {
        let clock = SimulationClock::default();
        let position = BattlefieldPosition(Vec2::ZERO);
        let memory = PerceptionMemory::default();
        let health = Health {
            current: 100,
            max: 100,
        };
        let inventory = inventory_with_ammo(1);

        let moving = synthesize_planner_state(
            &clock,
            &position,
            &health,
            &inventory,
            &memory,
            None,
            Some(&MovementOrder::MoveTo {
                target: PositionTarget::new(Vec2::new(1.0, 0.0), None),
            }),
            &[],
        );
        let holding = synthesize_planner_state(
            &clock,
            &position,
            &health,
            &inventory,
            &memory,
            None,
            Some(&MovementOrder::Hold),
            &[],
        );
        let no_order = synthesize_planner_state(
            &clock,
            &position,
            &health,
            &inventory,
            &memory,
            None,
            None,
            &[],
        );

        assert!(moving.has_move_target);
        assert!(!holding.has_move_target);
        assert!(!no_order.has_move_target);
    }

    #[test]
    fn headless_component_compatibility_synthesis() {
        let mut world = World::new();
        let hostile = world.spawn_empty().id();
        let unit = world
            .spawn((
                BattlefieldPosition(Vec2::new(1.0, 2.0)),
                Health {
                    current: 50,
                    max: 100,
                },
                inventory_with_ammo(5),
                AuditorySensor { range_m: 40.0 },
                MovementOrder::MoveTo {
                    target: PositionTarget::new(Vec2::new(3.0, 4.0), None),
                },
                PerceptionMemory {
                    contacts: vec![contact(
                        hostile,
                        Vec2::new(8.0, 2.0),
                        11,
                        0.9,
                        ContactKind::Visual,
                        ContactType::Hostile,
                        ReportedLifeStatus::Alive,
                    )],
                },
            ))
            .id();
        let clock = SimulationClock {
            tick: 11,
            ..default()
        };

        let position = world.get::<BattlefieldPosition>(unit).unwrap();
        let health = world.get::<Health>(unit).unwrap();
        let inventory = world.get::<Inventory>(unit).unwrap();
        let memory = world.get::<PerceptionMemory>(unit).unwrap();
        let auditory = world.get::<AuditorySensor>(unit);
        let order = world.get::<MovementOrder>(unit);

        let state = synthesize_planner_state(
            &clock,
            position,
            health,
            inventory,
            memory,
            auditory,
            order,
            &[shot_at(Vec2::new(2.0, 2.0))],
        );

        assert_eq!(state.position_m, Vec2::new(1.0, 2.0));
        assert_eq!(state.health_frac, 0.5);
        assert!(state.has_ammo);
        assert_eq!(state.nearest_hostile.unwrap().entity, hostile);
        assert!(state.under_fire);
        assert!(state.has_move_target);
        assert_eq!(state.tick, 11);
    }
}
