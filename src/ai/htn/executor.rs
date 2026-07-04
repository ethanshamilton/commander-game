use super::domain::{BoundOperator, Domain, TaskId};
use super::planner::{Plan, plan};
use super::synthesis::synthesize_planner_state;
use super::trace::{DecisionTrace, PlanRejectionReason, ReplanTrigger, TraceEvent};
use crate::GameState;
use crate::actors::units::{Alive, Health, Inventory, Soldier};
use crate::ai::perception::{AuditorySensor, ContactKind, PerceptionMemory};
use crate::gameplay::combat::{CombatOrder, ResolvedShot};
use crate::gameplay::simulation::{SimulationClock, SimulationSet, UnitOrder};
use crate::gameplay::spatial::BattlefieldPosition;
use bevy::prelude::*;

const FRESH_HOSTILE_TICKS: u64 = 1;
const MOVE_DESTINATION_EPSILON_M: f32 = 0.05;

pub struct HtnExecutorPlugin;

impl Plugin for HtnExecutorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HtnDomainRegistry>()
            .init_resource::<RecentResolvedShots>()
            .add_systems(
                FixedUpdate,
                (
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

#[derive(Resource, Default)]
pub struct HtnDomainRegistry {
    pub soldier: Option<Domain>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannerStateDigest {
    pub nearest_hostile: Option<Entity>,
    pub hostile_fresh: bool,
    pub health_band: u8,
    pub has_ammo: bool,
    pub under_fire: bool,
    pub has_move_target: bool,
}

impl PlannerStateDigest {
    pub fn from_state(state: &super::state::PlannerState) -> Self {
        Self {
            nearest_hostile: state.nearest_hostile.map(|hostile| hostile.entity),
            hostile_fresh: state.hostile_is_fresh(FRESH_HOSTILE_TICKS),
            health_band: health_band(state.health_frac),
            has_ammo: state.has_ammo,
            under_fire: state.under_fire,
            has_move_target: state.has_move_target,
        }
    }
}

fn health_band(health_frac: f32) -> u8 {
    if health_frac < 0.35 {
        0
    } else if health_frac < 0.7 {
        1
    } else {
        2
    }
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
    clock: Res<SimulationClock>,
    registry: Res<HtnDomainRegistry>,
    recent_shots: Res<RecentResolvedShots>,
    mut units: Query<
        (
            Entity,
            &BattlefieldPosition,
            &Health,
            &Inventory,
            &PerceptionMemory,
            Option<&AuditorySensor>,
            Option<&UnitOrder>,
            Option<&mut PlanRunner>,
            &mut DecisionTrace,
        ),
        (With<Soldier>, With<Alive>, With<Autonomous>),
    >,
) {
    let Some(domain) = registry.soldier.as_ref() else {
        return;
    };

    for (
        entity,
        position,
        health,
        inventory,
        memory,
        auditory_sensor,
        current_order,
        runner,
        mut trace,
    ) in &mut units
    {
        let state = synthesize_planner_state(
            &clock,
            position,
            health,
            inventory,
            memory,
            auditory_sensor,
            current_order,
            &recent_shots.shots,
        );
        let digest = PlannerStateDigest::from_state(&state);

        match runner {
            Some(mut runner) => {
                if runner.last_state_digest == digest {
                    continue;
                }

                let Some(candidate) = plan(domain, &state) else {
                    runner.last_state_digest = digest;
                    trace.push(TraceEvent::PlanRejected {
                        reason: PlanRejectionReason::NoValidPlan,
                    });
                    continue;
                };

                if !candidate.mtr.outranks(&runner.plan.mtr) {
                    runner.last_state_digest = digest;
                    trace.push(TraceEvent::PlanRejected {
                        reason: PlanRejectionReason::MtrNotBetter,
                    });
                    continue;
                }

                trace.push(TraceEvent::Replanned {
                    trigger: ReplanTrigger::RelevantStateChanged,
                });
                trace_plan_created(&mut trace, domain.root, domain, &candidate);
                *runner = PlanRunner {
                    plan: candidate,
                    current: 0,
                    step_state: StepState::Pending,
                    last_state_digest: digest,
                };
            }
            None => {
                if current_order.is_some() {
                    trace.push(TraceEvent::PlanRejected {
                        reason: PlanRejectionReason::ExternalOrderActive,
                    });
                    continue;
                }

                let Some(candidate) = plan(domain, &state) else {
                    trace.push(TraceEvent::PlanRejected {
                        reason: PlanRejectionReason::NoValidPlan,
                    });
                    continue;
                };

                trace.push(TraceEvent::Replanned {
                    trigger: ReplanTrigger::NoPlan,
                });
                trace_plan_created(&mut trace, domain.root, domain, &candidate);
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
    clock: Res<SimulationClock>,
    recent_shots: Res<RecentResolvedShots>,
    mut units: Query<
        (
            Entity,
            &BattlefieldPosition,
            &Health,
            &Inventory,
            &PerceptionMemory,
            Option<&AuditorySensor>,
            Option<&UnitOrder>,
            Option<&CombatOrder>,
            &mut PlanRunner,
            &mut DecisionTrace,
        ),
        (With<Soldier>, With<Alive>, With<Autonomous>),
    >,
) {
    for (
        entity,
        position,
        health,
        inventory,
        memory,
        auditory_sensor,
        current_order,
        _combat_order,
        mut runner,
        mut trace,
    ) in &mut units
    {
        if runner.step_state != StepState::Pending {
            continue;
        }

        if runner.current >= runner.plan.steps.len() {
            trace.push(TraceEvent::PlanCompleted);
            commands.entity(entity).remove::<PlanRunner>();
            continue;
        }

        let step = &runner.plan.steps[runner.current];
        if current_order.is_some() {
            trace.push(TraceEvent::PlanRejected {
                reason: PlanRejectionReason::ExternalOrderActive,
            });
            commands.entity(entity).remove::<PlanRunner>();
            continue;
        }

        let state = synthesize_planner_state(
            &clock,
            position,
            health,
            inventory,
            memory,
            auditory_sensor,
            current_order,
            &recent_shots.shots,
        );

        if !(step.preconditions)(&state) {
            trace.push(TraceEvent::StepFailed {
                task: step.task_name,
                failed_condition: "precondition failed before dispatch",
            });
            commands.entity(entity).remove::<PlanRunner>();
            continue;
        }

        match step.operator {
            BoundOperator::Hold => {
                commands
                    .entity(entity)
                    .insert((UnitOrder::Hold, CombatOrder::HoldFire));
            }
            BoundOperator::MoveTo { destination_m } => {
                commands
                    .entity(entity)
                    .insert(UnitOrder::MoveTo { destination_m });
            }
            BoundOperator::FireAt { target } => {
                commands
                    .entity(entity)
                    .insert(CombatOrder::FireAt { target });
            }
        }

        trace.push(TraceEvent::StepStarted {
            task: step.task_name,
            why: "primitive preconditions satisfied",
        });
        runner.step_state = StepState::Running;
    }
}

pub fn advance_plan_execution(
    mut commands: Commands,
    clock: Res<SimulationClock>,
    recent_shots: Res<RecentResolvedShots>,
    mut units: Query<
        (
            Entity,
            &BattlefieldPosition,
            &Health,
            &Inventory,
            &PerceptionMemory,
            Option<&AuditorySensor>,
            Option<&UnitOrder>,
            Option<&CombatOrder>,
            &mut PlanRunner,
            &mut DecisionTrace,
        ),
        (With<Soldier>, With<Alive>, With<Autonomous>),
    >,
) {
    for (
        entity,
        position,
        health,
        inventory,
        memory,
        auditory_sensor,
        current_order,
        combat_order,
        mut runner,
        mut trace,
    ) in &mut units
    {
        if runner.step_state != StepState::Running {
            continue;
        }

        if runner.current >= runner.plan.steps.len() {
            trace.push(TraceEvent::PlanCompleted);
            commands.entity(entity).remove::<PlanRunner>();
            continue;
        }

        let step = &runner.plan.steps[runner.current];
        let outcome = match step.operator {
            BoundOperator::Hold => StepPoll::Running,
            BoundOperator::MoveTo { destination_m } => poll_move(destination_m, current_order),
            BoundOperator::FireAt { target } => {
                let state = synthesize_planner_state(
                    &clock,
                    position,
                    health,
                    inventory,
                    memory,
                    auditory_sensor,
                    current_order,
                    &recent_shots.shots,
                );
                poll_fire(target, combat_order, &state)
            }
        };

        match outcome {
            StepPoll::Running => {}
            StepPoll::Succeeded => {
                runner.current += 1;
                if runner.current >= runner.plan.steps.len() {
                    trace.push(TraceEvent::PlanCompleted);
                    commands.entity(entity).remove::<PlanRunner>();
                } else {
                    runner.step_state = StepState::Pending;
                }
            }
            StepPoll::Failed(reason) => {
                trace.push(TraceEvent::StepFailed {
                    task: step.task_name,
                    failed_condition: reason,
                });
                commands.entity(entity).remove::<PlanRunner>();
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepPoll {
    Running,
    Succeeded,
    Failed(&'static str),
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

fn poll_fire(
    target: Entity,
    combat_order: Option<&CombatOrder>,
    state: &super::state::PlannerState,
) -> StepPoll {
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
                || !state.hostile_is_fresh(FRESH_HOSTILE_TICKS)
            {
                return StepPoll::Succeeded;
            }

            StepPoll::Running
        }
        _ => StepPoll::Succeeded,
    }
}

fn trace_plan_created(trace: &mut DecisionTrace, root: TaskId, domain: &Domain, plan: &Plan) {
    trace.push(TraceEvent::PlanCreated {
        root: domain.task_name(root).unwrap_or("<unknown>"),
        mtr: plan.mtr.clone(),
        steps: plan.steps.iter().map(|step| step.task_name).collect(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::units::{Allegiance, Item, ItemKind, Rank, Role, Side};
    use crate::ai::htn::domain::{DomainBuilder, Method, always, bind_hold, no_effect};

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

    fn spawn_autonomous(world: &mut World) -> Entity {
        world
            .spawn((
                Soldier {
                    rank: Rank::Private,
                    role: Role::Rifleman,
                },
                Alive,
                Autonomous,
                Allegiance { side: Side::Blue },
                BattlefieldPosition(Vec2::ZERO),
                Health {
                    current: 100,
                    max: 100,
                },
                inventory(10),
                PerceptionMemory::default(),
                DecisionTrace::default(),
            ))
            .id()
    }

    #[test]
    fn deliberation_creates_runner_when_no_plan() {
        let mut app = App::new();
        app.insert_resource(SimulationClock::default())
            .insert_resource(RecentResolvedShots::default())
            .insert_resource(HtnDomainRegistry {
                soldier: Some(test_domain()),
            })
            .add_systems(Update, deliberate_autonomous_units);

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

    #[test]
    fn pending_hold_dispatches_orders_and_starts_step() {
        let mut app = App::new();
        app.insert_resource(SimulationClock::default())
            .insert_resource(RecentResolvedShots::default())
            .add_systems(Update, start_pending_steps);

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
                BattlefieldPosition(Vec2::ZERO),
                Health {
                    current: 100,
                    max: 100,
                },
                inventory(10),
                PerceptionMemory::default(),
                DecisionTrace::default(),
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
                soldier: Some(domain),
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
            .add_systems(Update, deliberate_autonomous_units);

        let entity = app
            .world_mut()
            .spawn((
                Soldier {
                    rank: Rank::Private,
                    role: Role::Rifleman,
                },
                Alive,
                Autonomous,
                Allegiance { side: Side::Blue },
                BattlefieldPosition(Vec2::ZERO),
                Health {
                    current: 100,
                    max: 100,
                },
                inventory(10),
                AuditorySensor { range_m: 10.0 },
                PerceptionMemory::default(),
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
            ))
            .id();

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
                soldier: Some(domain),
            })
            .insert_resource(RecentResolvedShots::default())
            .add_systems(Update, deliberate_autonomous_units);

        let entity = app
            .world_mut()
            .spawn((
                Soldier {
                    rank: Rank::Private,
                    role: Role::Rifleman,
                },
                Alive,
                Autonomous,
                Allegiance { side: Side::Blue },
                BattlefieldPosition(Vec2::ZERO),
                Health {
                    current: 100,
                    max: 100,
                },
                inventory(10),
                PerceptionMemory::default(),
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
            ))
            .id();

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
        app.insert_resource(SimulationClock::default())
            .insert_resource(RecentResolvedShots::default())
            .add_systems(Update, advance_plan_execution);

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
                BattlefieldPosition(Vec2::ZERO),
                Health {
                    current: 100,
                    max: 100,
                },
                inventory(10),
                PerceptionMemory::default(),
                DecisionTrace::default(),
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
}
