use super::domain::{Domain, TaskId};
use super::operators::StepPoll;
use super::planner::{Plan, plan};
use super::state::PlannerStateDigest;
use super::synthesis::PlannerBelief;
use super::trace::{DecisionTrace, PlanRejectionReason, ReplanTrigger, TraceEvent};
use crate::GameState;
use crate::actors::units::{Alive, Soldier};
use crate::gameplay::combat::{CombatOrder, ResolvedShot};
use crate::gameplay::orders::{
    CombatOrderSource, UnitOrderSource, clear_if_htn, is_player_sourced,
};
use crate::gameplay::simulation::{SimulationSet, UnitOrder};
use bevy::prelude::*;
use std::collections::HashMap;

pub struct HtnExecutorPlugin;

impl Plugin for HtnExecutorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HtnDomainRegistry>()
            .init_resource::<RecentResolvedShots>()
            .add_systems(
                FixedUpdate,
                (
                    super::synthesis::synthesize_beliefs,
                    advance_plan_execution,
                    deliberate_autonomous_units,
                    start_pending_steps,
                )
                    .chain()
                    .in_set(SimulationSet::Thinking)
                    .run_if(in_state(GameState::MissionScreen)),
            )
            .add_systems(
                FixedUpdate,
                collect_recent_resolved_shots.in_set(SimulationSet::Cleanup),
            );
    }
}

/// Identifies which HTN domain an autonomous unit plans with. New archetypes
/// (vehicles, squads, ...) add a variant here plus a builder registered in
/// `HtnPlugin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DomainId {
    Soldier,
}

/// Which HTN domain this autonomous unit plans with.
#[derive(Component, Debug, Clone, Copy)]
pub struct DomainRef(pub DomainId);

#[derive(Resource, Default)]
pub struct HtnDomainRegistry {
    pub domains: HashMap<DomainId, Domain>,
}

#[derive(Resource, Debug, Default, Clone)]
pub struct RecentResolvedShots {
    pub shots: Vec<ResolvedShot>,
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct Autonomous;

#[derive(Component, Debug, Clone)]
pub struct PlanRunner {
    pub plan: Plan,
    pub current: usize,
    pub step_state: StepState,
    pub last_state_digest: PlannerStateDigest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Pending,
    Running,
}

pub fn collect_recent_resolved_shots(
    mut reader: MessageReader<ResolvedShot>,
    mut recent: ResMut<RecentResolvedShots>,
) {
    recent.shots.clear();
    recent.shots.extend(reader.read().copied());
}

pub fn deliberate_autonomous_units(
    mut commands: Commands,
    clock: Option<Res<crate::gameplay::simulation::SimulationClock>>,
    registry: Res<HtnDomainRegistry>,
    mut units: Query<
        (
            Entity,
            &PlannerBelief,
            &DomainRef,
            Option<&UnitOrderSource>,
            Option<&CombatOrderSource>,
            Option<&mut PlanRunner>,
            &mut DecisionTrace,
        ),
        (With<Soldier>, With<Alive>, With<Autonomous>),
    >,
) {
    let (trace_tick, trace_elapsed_s) = trace_time(clock);

    for (entity, belief, domain_ref, unit_source, combat_source, runner, mut trace) in &mut units {
        let Some(domain) = registry.domains.get(&domain_ref.0) else {
            warn!(?entity, domain = ?domain_ref.0, "no HTN domain registered for unit's DomainRef");
            continue;
        };
        let state = &belief.state;
        let digest = PlannerStateDigest::from_state(state);

        match runner {
            Some(mut runner) => {
                if has_external_order(unit_source, combat_source) {
                    trace.push(
                        trace_tick,
                        trace_elapsed_s,
                        TraceEvent::PlanRejected {
                            reason: PlanRejectionReason::ExternalOrderActive,
                        },
                    );
                    commands.entity(entity).remove::<PlanRunner>();
                    continue;
                }

                if runner.last_state_digest == digest {
                    continue;
                }

                let Some(candidate) = plan(domain, state) else {
                    runner.last_state_digest = digest;
                    trace.push(
                        trace_tick,
                        trace_elapsed_s,
                        TraceEvent::PlanRejected {
                            reason: PlanRejectionReason::NoValidPlan,
                        },
                    );
                    continue;
                };

                if !should_adopt_candidate(&candidate, &runner.plan) {
                    runner.last_state_digest = digest;
                    trace.push(
                        trace_tick,
                        trace_elapsed_s,
                        TraceEvent::PlanRejected {
                            reason: PlanRejectionReason::MtrNotBetter,
                        },
                    );
                    continue;
                }

                trace.push(
                    trace_tick,
                    trace_elapsed_s,
                    TraceEvent::Replanned {
                        trigger: ReplanTrigger::RelevantStateChanged,
                    },
                );
                clear_if_htn::<UnitOrder>(&mut commands, entity, unit_source);
                clear_if_htn::<CombatOrder>(&mut commands, entity, combat_source);
                trace_plan_created(
                    &mut trace,
                    trace_tick,
                    trace_elapsed_s,
                    domain.root,
                    domain,
                    &candidate,
                );
                *runner = PlanRunner {
                    plan: candidate,
                    current: 0,
                    step_state: StepState::Pending,
                    last_state_digest: digest,
                };
            }
            None => {
                if has_external_order(unit_source, combat_source) {
                    trace.push(
                        trace_tick,
                        trace_elapsed_s,
                        TraceEvent::PlanRejected {
                            reason: PlanRejectionReason::ExternalOrderActive,
                        },
                    );
                    continue;
                }

                let Some(candidate) = plan(domain, state) else {
                    trace.push(
                        trace_tick,
                        trace_elapsed_s,
                        TraceEvent::PlanRejected {
                            reason: PlanRejectionReason::NoValidPlan,
                        },
                    );
                    continue;
                };

                trace.push(
                    trace_tick,
                    trace_elapsed_s,
                    TraceEvent::Replanned {
                        trigger: ReplanTrigger::NoPlan,
                    },
                );
                trace_plan_created(
                    &mut trace,
                    trace_tick,
                    trace_elapsed_s,
                    domain.root,
                    domain,
                    &candidate,
                );
                commands.entity(entity).insert(PlanRunner {
                    plan: candidate,
                    current: 0,
                    step_state: StepState::Pending,
                    last_state_digest: digest,
                });
            }
        }
    }
}

pub fn start_pending_steps(
    mut commands: Commands,
    clock: Option<Res<crate::gameplay::simulation::SimulationClock>>,
    mut units: Query<
        (Entity, &PlannerBelief, &mut PlanRunner, &mut DecisionTrace),
        (With<Soldier>, With<Alive>, With<Autonomous>),
    >,
) {
    let (trace_tick, trace_elapsed_s) = trace_time(clock);

    for (entity, belief, mut runner, mut trace) in &mut units {
        if runner.step_state != StepState::Pending {
            continue;
        }

        if runner.current >= runner.plan.steps.len() {
            trace.push(trace_tick, trace_elapsed_s, TraceEvent::PlanCompleted);
            commands.entity(entity).remove::<PlanRunner>();
            continue;
        }

        let step = runner.plan.steps[runner.current].clone();

        if !(step.preconditions)(&belief.state) {
            trace.push(
                trace_tick,
                trace_elapsed_s,
                TraceEvent::StepFailed {
                    task: step.task_name,
                    failed_condition: "precondition failed before dispatch",
                },
            );
            commands.entity(entity).remove::<PlanRunner>();
            continue;
        }

        step.operator.dispatch(&mut commands, entity);

        trace.push(
            trace_tick,
            trace_elapsed_s,
            TraceEvent::StepStarted {
                task: step.task_name,
                why: step.reason,
                operator: step.operator.describe(),
            },
        );
        runner.step_state = StepState::Running;
    }
}

pub fn advance_plan_execution(
    mut commands: Commands,
    clock: Option<Res<crate::gameplay::simulation::SimulationClock>>,
    mut units: Query<
        (
            Entity,
            &PlannerBelief,
            Option<&UnitOrder>,
            Option<&CombatOrder>,
            Option<&UnitOrderSource>,
            Option<&CombatOrderSource>,
            &mut PlanRunner,
            &mut DecisionTrace,
        ),
        (With<Soldier>, With<Alive>, With<Autonomous>),
    >,
) {
    let (trace_tick, trace_elapsed_s) = trace_time(clock);

    for (
        entity,
        belief,
        current_order,
        combat_order,
        unit_source,
        combat_source,
        mut runner,
        mut trace,
    ) in &mut units
    {
        if runner.step_state != StepState::Running {
            continue;
        }

        if runner.current >= runner.plan.steps.len() {
            trace.push(trace_tick, trace_elapsed_s, TraceEvent::PlanCompleted);
            clear_if_htn::<UnitOrder>(&mut commands, entity, unit_source);
            clear_if_htn::<CombatOrder>(&mut commands, entity, combat_source);
            commands.entity(entity).remove::<PlanRunner>();
            continue;
        }

        let step = &runner.plan.steps[runner.current];
        let outcome = step
            .operator
            .poll(&belief.state, current_order, combat_order);

        match outcome {
            StepPoll::Running => {}
            StepPoll::Succeeded => {
                clear_if_htn::<UnitOrder>(&mut commands, entity, unit_source);
                clear_if_htn::<CombatOrder>(&mut commands, entity, combat_source);
                runner.current += 1;
                if runner.current >= runner.plan.steps.len() {
                    trace.push(trace_tick, trace_elapsed_s, TraceEvent::PlanCompleted);
                    commands.entity(entity).remove::<PlanRunner>();
                } else {
                    runner.step_state = StepState::Pending;
                }
            }
            StepPoll::Failed(reason) => {
                trace.push(
                    trace_tick,
                    trace_elapsed_s,
                    TraceEvent::StepFailed {
                        task: step.task_name,
                        failed_condition: reason,
                    },
                );
                clear_if_htn::<UnitOrder>(&mut commands, entity, unit_source);
                clear_if_htn::<CombatOrder>(&mut commands, entity, combat_source);
                commands.entity(entity).remove::<PlanRunner>();
            }
        }
    }
}

fn trace_time(clock: Option<Res<crate::gameplay::simulation::SimulationClock>>) -> (u64, f32) {
    clock
        .as_deref()
        .map(|clock| (clock.tick, clock.elapsed_s))
        .unwrap_or((0, 0.0))
}

fn should_adopt_candidate(candidate: &Plan, current: &Plan) -> bool {
    candidate.mtr.outranks(&current.mtr)
        || (candidate.mtr == current.mtr && plan_bound_operators_differ(candidate, current))
}

fn plan_bound_operators_differ(a: &Plan, b: &Plan) -> bool {
    a.steps.len() != b.steps.len()
        || a.steps
            .iter()
            .zip(&b.steps)
            .any(|(a_step, b_step)| a_step.operator != b_step.operator)
}

/// True if either order component present on the entity was issued directly
/// by the player. Player-sourced orders preempt autonomous planning; Htn- and
/// Doctrine-sourced orders never do (see `docs/gameplay/orders.md`).
fn has_external_order(
    unit_source: Option<&UnitOrderSource>,
    combat_source: Option<&CombatOrderSource>,
) -> bool {
    is_player_sourced(unit_source) || is_player_sourced(combat_source)
}

fn trace_plan_created(
    trace: &mut DecisionTrace,
    tick: u64,
    elapsed_s: f32,
    root: TaskId,
    domain: &Domain,
    plan: &Plan,
) {
    trace.push(
        tick,
        elapsed_s,
        TraceEvent::PlanCreated {
            root: domain.task_name(root).unwrap_or("<unknown>"),
            mtr: plan.mtr.clone(),
            steps: plan.steps.iter().map(|step| step.describe()).collect(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::units::{Allegiance, Health, Inventory, Item, ItemKind, Rank, Role, Side};
    use crate::ai::htn::domain::{
        DomainBuilder, Method, always, bind_fire_at_nearest_hostile, bind_hold, no_effect,
    };
    use crate::ai::htn::operators::BoundOperator;
    use crate::ai::htn::state::{HostileBelief, PlannerState};
    use crate::ai::htn::synthesis::synthesize_beliefs;
    use crate::ai::perception::{Contact, ContactKind, ContactType, PerceptionMemory};
    use crate::gameplay::simulation::SimulationClock;
    use crate::gameplay::spatial::BattlefieldPosition;
    use crate::intel::ReportedLifeStatus;

    fn inventory(ammo: u32) -> Inventory {
        Inventory {
            items: vec![Item {
                kind: ItemKind::Ammo,
                count: ammo,
            }],
        }
    }

    fn under_fire(state: &super::super::state::PlannerState) -> bool {
        state.under_fire
    }

    fn test_domain() -> Domain {
        let mut builder = DomainBuilder::new();
        let hold = builder.primitive("Hold", always, bind_hold, no_effect);
        let root = builder.compound(
            "Root",
            vec![Method {
                name: "Idle",
                preconditions: always,
                subtasks: vec![hold],
            }],
        );
        builder.build(root)
    }

    fn priority_domain() -> Domain {
        let mut builder = DomainBuilder::new();
        let urgent = builder.primitive("Urgent", always, bind_hold, no_effect);
        let idle = builder.primitive("Idle", always, bind_hold, no_effect);
        let root = builder.compound(
            "Root",
            vec![
                Method {
                    name: "UrgentWhenUnderFire",
                    preconditions: under_fire,
                    subtasks: vec![urgent],
                },
                Method {
                    name: "Idle",
                    preconditions: always,
                    subtasks: vec![idle],
                },
            ],
        );
        builder.build(root)
    }

    /// Bundle every component `synthesize_beliefs` reads, so tests can run the
    /// real synthesis system instead of hand-building `PlannerBelief`.
    fn belief_inputs(
        world: &mut World,
        entity: Entity,
        health: Health,
        inventory: Inventory,
        memory: PerceptionMemory,
    ) {
        world
            .entity_mut(entity)
            .insert((health, inventory, memory, PlannerBelief::default()));
    }

    fn spawn_autonomous(world: &mut World) -> Entity {
        let entity = world
            .spawn((
                Soldier {
                    rank: Rank::Private,
                    role: Role::Rifleman,
                },
                Alive,
                Autonomous,
                DomainRef(DomainId::Soldier),
                Allegiance { side: Side::Blue },
                BattlefieldPosition(Vec2::ZERO),
                DecisionTrace::default(),
            ))
            .id();
        belief_inputs(
            world,
            entity,
            Health {
                current: 100,
                max: 100,
            },
            inventory(10),
            PerceptionMemory::default(),
        );
        entity
    }

    #[test]
    fn deliberation_creates_runner_when_no_plan() {
        let mut app = App::new();
        app.insert_resource(SimulationClock::default())
            .insert_resource(RecentResolvedShots::default())
            .insert_resource(HtnDomainRegistry {
                domains: HashMap::from([(DomainId::Soldier, test_domain())]),
            })
            .add_systems(
                Update,
                (synthesize_beliefs, deliberate_autonomous_units).chain(),
            );

        let entity = spawn_autonomous(app.world_mut());
        app.update();

        assert!(app.world().get::<PlanRunner>(entity).is_some());
        let trace = app.world().get::<DecisionTrace>(entity).unwrap();
        assert!(
            trace
                .events()
                .any(|event| matches!(event, TraceEvent::PlanCreated { root: "Root", .. }))
        );
    }

    /// Regression test: real soldiers spawn with `CombatOrder::HoldFire` as their
    /// default posture (see `spawn_soldier_at`), tagged `CombatOrderSource::doctrine()`.
    /// That default must not read as an "external order" that suppresses autonomous
    /// planning, or every autonomous unit is permanently inert.
    #[test]
    fn deliberation_creates_runner_despite_default_hold_fire_posture() {
        let mut app = App::new();
        app.insert_resource(SimulationClock::default())
            .insert_resource(RecentResolvedShots::default())
            .insert_resource(HtnDomainRegistry {
                domains: HashMap::from([(DomainId::Soldier, test_domain())]),
            })
            .add_systems(
                Update,
                (synthesize_beliefs, deliberate_autonomous_units).chain(),
            );

        let entity = spawn_autonomous(app.world_mut());
        app.world_mut()
            .entity_mut(entity)
            .insert((CombatOrder::HoldFire, CombatOrderSource::doctrine()));
        app.update();

        assert!(
            app.world().get::<PlanRunner>(entity).is_some(),
            "default HoldFire posture must not block plan creation"
        );
        let trace = app.world().get::<DecisionTrace>(entity).unwrap();
        assert!(
            !trace.events().any(|event| matches!(
                event,
                TraceEvent::PlanRejected {
                    reason: PlanRejectionReason::ExternalOrderActive
                }
            )),
            "default HoldFire posture must not be treated as an external order"
        );
    }

    #[test]
    fn pending_hold_dispatches_orders_and_starts_step() {
        let mut app = App::new();
        app.add_systems(Update, start_pending_steps);

        let domain = test_domain();
        let plan = plan(&domain, &super::super::state::PlannerState::default()).unwrap();
        let entity = app
            .world_mut()
            .spawn((
                Soldier {
                    rank: Rank::Private,
                    role: Role::Rifleman,
                },
                Alive,
                Autonomous,
                DecisionTrace::default(),
                PlannerBelief::default(),
                PlanRunner {
                    plan,
                    current: 0,
                    step_state: StepState::Pending,
                    last_state_digest: PlannerStateDigest {
                        nearest_hostile: None,
                        hostile_fresh: false,
                        health_band: 2,
                        has_ammo: true,
                        under_fire: false,
                        has_move_target: false,
                    },
                },
            ))
            .id();

        app.update();

        assert!(matches!(
            app.world().get::<UnitOrder>(entity).unwrap(),
            UnitOrder::Hold
        ));
        assert!(matches!(
            app.world().get::<CombatOrder>(entity).unwrap(),
            CombatOrder::HoldFire
        ));
        assert_eq!(
            app.world().get::<UnitOrderSource>(entity).unwrap().source,
            crate::gameplay::orders::OrderSource::Htn
        );
        assert_eq!(
            app.world().get::<CombatOrderSource>(entity).unwrap().source,
            crate::gameplay::orders::OrderSource::Htn
        );
        assert_eq!(
            app.world().get::<PlanRunner>(entity).unwrap().step_state,
            StepState::Running
        );
    }

    #[test]
    fn relevant_state_change_adopts_higher_priority_mtr() {
        let domain = priority_domain();
        let old_plan = plan(&domain, &super::super::state::PlannerState::default()).unwrap();
        assert_eq!(old_plan.mtr, super::super::planner::Mtr(vec![1]));

        let mut app = App::new();
        app.insert_resource(SimulationClock::default())
            .insert_resource(HtnDomainRegistry {
                domains: HashMap::from([(DomainId::Soldier, domain)]),
            })
            .insert_resource(RecentResolvedShots {
                shots: vec![ResolvedShot {
                    shooter: Entity::from_raw_u32(1).unwrap(),
                    target: Entity::from_raw_u32(2).unwrap(),
                    shooter_position_m: Vec2::ZERO,
                    target_position_m: Vec2::ZERO,
                    impact_position_m: Vec2::ZERO,
                    hit: false,
                    damage: 0,
                    projectile_speed_mps: 700.0,
                    tracer_length_m: 8.0,
                }],
            })
            .add_systems(
                Update,
                (synthesize_beliefs, deliberate_autonomous_units).chain(),
            );

        let entity = app.world_mut().spawn((
            Soldier {
                rank: Rank::Private,
                role: Role::Rifleman,
            },
            Alive,
            Autonomous,
            DomainRef(DomainId::Soldier),
            Allegiance { side: Side::Blue },
            BattlefieldPosition(Vec2::ZERO),
            crate::ai::perception::AuditorySensor { range_m: 10.0 },
            DecisionTrace::default(),
            PlanRunner {
                plan: old_plan,
                current: 0,
                step_state: StepState::Running,
                last_state_digest: PlannerStateDigest {
                    nearest_hostile: None,
                    hostile_fresh: false,
                    health_band: 2,
                    has_ammo: true,
                    under_fire: false,
                    has_move_target: false,
                },
            },
        ));
        let entity = entity.id();
        belief_inputs(
            app.world_mut(),
            entity,
            Health {
                current: 100,
                max: 100,
            },
            inventory(10),
            PerceptionMemory::default(),
        );

        app.update();

        assert_eq!(
            app.world().get::<PlanRunner>(entity).unwrap().plan.mtr,
            super::super::planner::Mtr(vec![0])
        );
    }

    #[test]
    fn relevant_state_change_rejects_lower_priority_mtr() {
        let domain = priority_domain();
        let high_priority_state = super::super::state::PlannerState {
            under_fire: true,
            ..default()
        };
        let old_plan = plan(&domain, &high_priority_state).unwrap();
        assert_eq!(old_plan.mtr, super::super::planner::Mtr(vec![0]));

        let mut app = App::new();
        app.insert_resource(SimulationClock::default())
            .insert_resource(HtnDomainRegistry {
                domains: HashMap::from([(DomainId::Soldier, domain)]),
            })
            .insert_resource(RecentResolvedShots::default())
            .add_systems(
                Update,
                (synthesize_beliefs, deliberate_autonomous_units).chain(),
            );

        let entity = app.world_mut().spawn((
            Soldier {
                rank: Rank::Private,
                role: Role::Rifleman,
            },
            Alive,
            Autonomous,
            DomainRef(DomainId::Soldier),
            Allegiance { side: Side::Blue },
            BattlefieldPosition(Vec2::ZERO),
            DecisionTrace::default(),
            PlanRunner {
                plan: old_plan,
                current: 0,
                step_state: StepState::Running,
                last_state_digest: PlannerStateDigest {
                    nearest_hostile: None,
                    hostile_fresh: false,
                    health_band: 2,
                    has_ammo: true,
                    under_fire: true,
                    has_move_target: false,
                },
            },
        ));
        let entity = entity.id();
        belief_inputs(
            app.world_mut(),
            entity,
            Health {
                current: 100,
                max: 100,
            },
            inventory(10),
            PerceptionMemory::default(),
        );
        // Force `under_fire` in the synthesized belief by seeding a nearby
        // fresh hostile contact (see synthesis::under_fire_from_contacts).
        app.world_mut().entity_mut(entity).insert(PerceptionMemory {
            contacts: vec![Contact {
                target: Entity::from_raw_u32(99).unwrap(),
                last_seen_position_m: Vec2::new(1.0, 0.0),
                last_seen_time_s: 0.0,
                last_seen_tick: 0,
                confidence: 1.0,
                observed_life_status: ReportedLifeStatus::Alive,
                kind: ContactKind::Visual,
                contact_type: ContactType::Hostile,
            }],
        });

        app.update();

        assert_eq!(
            app.world().get::<PlanRunner>(entity).unwrap().plan.mtr,
            super::super::planner::Mtr(vec![0])
        );
        assert!(
            app.world()
                .get::<DecisionTrace>(entity)
                .unwrap()
                .events()
                .any(|event| matches!(
                    event,
                    TraceEvent::PlanRejected {
                        reason: PlanRejectionReason::MtrNotBetter
                    }
                ))
        );
    }

    #[test]
    fn running_move_succeeds_when_order_removed() {
        let mut app = App::new();
        app.add_systems(Update, advance_plan_execution);

        let mut builder = DomainBuilder::new();
        let mov = builder.primitive(
            "Move",
            always,
            |_| {
                Some(BoundOperator::MoveTo {
                    destination_m: Vec2::new(1.0, 0.0),
                })
            },
            no_effect,
        );
        let domain = builder.build(mov);
        let plan = plan(&domain, &super::super::state::PlannerState::default()).unwrap();
        let entity = app
            .world_mut()
            .spawn((
                Soldier {
                    rank: Rank::Private,
                    role: Role::Rifleman,
                },
                Alive,
                Autonomous,
                DecisionTrace::default(),
                PlannerBelief::default(),
                PlanRunner {
                    plan,
                    current: 0,
                    step_state: StepState::Running,
                    last_state_digest: PlannerStateDigest {
                        nearest_hostile: None,
                        hostile_fresh: false,
                        health_band: 2,
                        has_ammo: true,
                        under_fire: false,
                        has_move_target: false,
                    },
                },
            ))
            .id();

        app.update();

        assert!(app.world().get::<PlanRunner>(entity).is_none());
        assert!(
            app.world()
                .get::<DecisionTrace>(entity)
                .unwrap()
                .events()
                .any(|event| matches!(event, TraceEvent::PlanCompleted))
        );
    }

    #[test]
    fn player_unit_order_replaces_htn_order_without_being_cleared() {
        let mut app = App::new();
        app.insert_resource(SimulationClock::default())
            .insert_resource(RecentResolvedShots::default())
            .insert_resource(HtnDomainRegistry {
                domains: HashMap::from([(DomainId::Soldier, test_domain())]),
            })
            .add_systems(
                Update,
                (synthesize_beliefs, deliberate_autonomous_units).chain(),
            );

        let mut builder = DomainBuilder::new();
        let mov = builder.primitive(
            "Move",
            always,
            |_| {
                Some(BoundOperator::MoveTo {
                    destination_m: Vec2::new(1.0, 0.0),
                })
            },
            no_effect,
        );
        let domain = builder.build(mov);
        let plan = plan(&domain, &PlannerState::default()).unwrap();
        let player_order = UnitOrder::MoveTo {
            destination_m: Vec2::new(5.0, 0.0),
        };
        let entity = app.world_mut().spawn((
            Soldier {
                rank: Rank::Private,
                role: Role::Rifleman,
            },
            Alive,
            Autonomous,
            DomainRef(DomainId::Soldier),
            Allegiance { side: Side::Blue },
            BattlefieldPosition(Vec2::ZERO),
            DecisionTrace::default(),
            player_order,
            UnitOrderSource::player(),
            PlanRunner {
                plan,
                current: 0,
                step_state: StepState::Running,
                last_state_digest: PlannerStateDigest {
                    nearest_hostile: None,
                    hostile_fresh: false,
                    health_band: 2,
                    has_ammo: true,
                    under_fire: false,
                    has_move_target: true,
                },
            },
        ));
        let entity = entity.id();
        belief_inputs(
            app.world_mut(),
            entity,
            Health {
                current: 100,
                max: 100,
            },
            inventory(10),
            PerceptionMemory::default(),
        );

        app.update();

        assert!(app.world().get::<PlanRunner>(entity).is_none());
        assert_eq!(app.world().get::<UnitOrder>(entity), Some(&player_order));
        assert_eq!(
            app.world().get::<UnitOrderSource>(entity).unwrap().source,
            crate::gameplay::orders::OrderSource::Player
        );
        assert!(
            app.world()
                .get::<DecisionTrace>(entity)
                .unwrap()
                .events()
                .any(|event| matches!(
                    event,
                    TraceEvent::PlanRejected {
                        reason: PlanRejectionReason::ExternalOrderActive
                    }
                ))
        );
    }

    /// An entity that has an autonomous plan running and a player-sourced
    /// `UnitOrder` must have its runner torn down without the order (or its
    /// provenance) being cleared.
    #[test]
    fn deliberation_removes_runner_but_preserves_player_order_and_source() {
        let mut app = App::new();
        app.insert_resource(SimulationClock::default())
            .insert_resource(RecentResolvedShots::default())
            .insert_resource(HtnDomainRegistry {
                domains: HashMap::from([(DomainId::Soldier, test_domain())]),
            })
            .add_systems(
                Update,
                (synthesize_beliefs, deliberate_autonomous_units).chain(),
            );

        let domain = test_domain();
        let running_plan = plan(&domain, &PlannerState::default()).unwrap();
        let player_order = UnitOrder::MoveTo {
            destination_m: Vec2::new(3.0, 4.0),
        };

        let entity = app.world_mut().spawn((
            Soldier {
                rank: Rank::Private,
                role: Role::Rifleman,
            },
            Alive,
            Autonomous,
            DomainRef(DomainId::Soldier),
            Allegiance { side: Side::Blue },
            BattlefieldPosition(Vec2::ZERO),
            DecisionTrace::default(),
            player_order,
            UnitOrderSource::player(),
            PlanRunner {
                plan: running_plan,
                current: 0,
                step_state: StepState::Running,
                last_state_digest: PlannerStateDigest {
                    nearest_hostile: None,
                    hostile_fresh: false,
                    health_band: 2,
                    has_ammo: true,
                    under_fire: false,
                    has_move_target: true,
                },
            },
        ));
        let entity = entity.id();
        belief_inputs(
            app.world_mut(),
            entity,
            Health {
                current: 100,
                max: 100,
            },
            inventory(10),
            PerceptionMemory::default(),
        );

        app.update();

        assert!(app.world().get::<PlanRunner>(entity).is_none());
        assert_eq!(app.world().get::<UnitOrder>(entity), Some(&player_order));
        assert!(app.world().get::<UnitOrderSource>(entity).is_some());
    }

    #[test]
    fn equal_mtr_candidate_with_different_bound_operator_is_adopted() {
        fn engage_domain() -> Domain {
            let mut builder = DomainBuilder::new();
            let fire = builder.primitive(
                "FireAtNearestHostile",
                always,
                bind_fire_at_nearest_hostile,
                no_effect,
            );
            let root = builder.compound(
                "Root",
                vec![Method {
                    name: "Engage",
                    preconditions: always,
                    subtasks: vec![fire],
                }],
            );
            builder.build(root)
        }

        // Entity id used only to build `old_plan`'s bound operator; a distinct,
        // real `target_a` is spawned below and used on the actual test entity.
        let placeholder_target = Entity::from_raw_u32(1).unwrap();
        let domain = engage_domain();
        let old_plan = plan(
            &domain,
            &PlannerState {
                nearest_hostile: Some(HostileBelief {
                    entity: placeholder_target,
                    position_m: Vec2::new(1.0, 0.0),
                    confidence: 1.0,
                    last_seen_tick: 0,
                    kind: ContactKind::Visual,
                }),
                has_ammo: true,
                ..default()
            },
        )
        .unwrap();
        assert_eq!(old_plan.mtr, super::super::planner::Mtr(vec![0]));

        let mut app = App::new();
        app.insert_resource(SimulationClock {
            tick: 1,
            ..default()
        })
        .insert_resource(RecentResolvedShots::default())
        .insert_resource(HtnDomainRegistry {
            domains: HashMap::from([(DomainId::Soldier, domain)]),
        })
        .add_systems(
            Update,
            (synthesize_beliefs, deliberate_autonomous_units).chain(),
        );

        let target_a = app.world_mut().spawn_empty().id();
        let target_b = app.world_mut().spawn_empty().id();

        let entity = app.world_mut().spawn((
            Soldier {
                rank: Rank::Private,
                role: Role::Rifleman,
            },
            Alive,
            Autonomous,
            DomainRef(DomainId::Soldier),
            Allegiance { side: Side::Blue },
            BattlefieldPosition(Vec2::ZERO),
            DecisionTrace::default(),
            CombatOrder::FireAt { target: target_a },
            CombatOrderSource::htn(),
            PlanRunner {
                plan: old_plan,
                current: 0,
                step_state: StepState::Running,
                last_state_digest: PlannerStateDigest {
                    nearest_hostile: Some(target_a),
                    hostile_fresh: true,
                    health_band: 2,
                    has_ammo: true,
                    under_fire: false,
                    has_move_target: false,
                },
            },
        ));
        let entity = entity.id();
        belief_inputs(
            app.world_mut(),
            entity,
            Health {
                current: 100,
                max: 100,
            },
            inventory(10),
            PerceptionMemory {
                contacts: vec![Contact {
                    target: target_b,
                    last_seen_position_m: Vec2::new(2.0, 0.0),
                    last_seen_time_s: 0.0,
                    last_seen_tick: 1,
                    confidence: 1.0,
                    observed_life_status: ReportedLifeStatus::Alive,
                    kind: ContactKind::Visual,
                    contact_type: ContactType::Hostile,
                }],
            },
        );

        app.update();

        let runner = app.world().get::<PlanRunner>(entity).unwrap();
        assert_eq!(runner.plan.mtr, super::super::planner::Mtr(vec![0]));
        assert_eq!(
            runner.plan.steps[0].operator,
            BoundOperator::FireAt { target: target_b }
        );
        assert_eq!(runner.step_state, StepState::Pending);
        assert!(app.world().get::<CombatOrder>(entity).is_none());
        assert!(app.world().get::<CombatOrderSource>(entity).is_none());
    }
}
