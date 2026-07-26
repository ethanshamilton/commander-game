#![doc = include_str!("../../docs/gameplay/command_succession.md")]

use crate::GameState;
use crate::actors::units::{Alive, Allegiance, Soldier};
use crate::gameplay::command::{CommandForest, SuccessionOutcome, UnitIdentity};
use crate::gameplay::debug_powers::DebugPowersSet;
use crate::gameplay::lifecycle::{UnitDeathCause, UnitDied};
use crate::gameplay::objectives::{MissionEnded, MissionOutcome, transition_mission_outcome};
use crate::gameplay::simulation::{SimulationClock, SimulationSet};
use crate::gameplay::squads::{MemberOfSquad, Squad};
use crate::player::knowledge::PlayerControlledUnit;
use crate::player::selection::SelectedUnit;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use std::cmp::Ordering;
use std::collections::HashSet;

#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct CommandStructureChanged {
    pub tick: u64,
    pub outcome: SuccessionOutcome,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSucceeded {
    pub squad: Entity,
    pub deceased: Entity,
    pub successor: Option<Entity>,
    pub tick: u64,
    pub squad_revision: u64,
}

pub(crate) fn register_command_succession(app: &mut App) {
    app.add_message::<CommandStructureChanged>()
        .add_message::<CommandSucceeded>()
        .add_systems(
            FixedUpdate,
            process_combat_deaths.in_set(SimulationSet::Cleanup),
        )
        .add_systems(
            Update,
            process_debug_deaths
                .after(DebugPowersSet::DeathCommands)
                .run_if(in_state(GameState::MissionScreen)),
        );
}

#[derive(SystemParam)]
struct SuccessionContext<'w, 's> {
    forest: ResMut<'w, CommandForest>,
    memberships: Query<'w, 's, &'static MemberOfSquad>,
    squads: Query<'w, 's, &'static mut Squad>,
    eligible_units: Query<'w, 's, &'static Allegiance, (With<Soldier>, With<Alive>)>,
    identities: Query<'w, 's, &'static UnitIdentity>,
    player_controlled: Query<'w, 's, (), With<PlayerControlledUnit>>,
    selected: ResMut<'w, SelectedUnit>,
    structure_changed: MessageWriter<'w, CommandStructureChanged>,
    command_succeeded: MessageWriter<'w, CommandSucceeded>,
}

fn process_combat_deaths(mut deaths: MessageReader<UnitDied>, mut context: SuccessionContext) {
    let deaths = deaths
        .read()
        .copied()
        .filter(|death| matches!(death.cause, UnitDeathCause::Combat { .. }))
        .collect();
    process_death_batch(deaths, &mut context);
}

fn process_debug_deaths(
    mut deaths: MessageReader<UnitDied>,
    mut context: SuccessionContext,
    mut outcome: ResMut<MissionOutcome>,
    mut clock: ResMut<SimulationClock>,
    mut mission_ended: MessageWriter<MissionEnded>,
) {
    let deaths: Vec<_> = deaths
        .read()
        .copied()
        .filter(|death| death.cause == UnitDeathCause::Debug)
        .collect();
    let player_died = deaths
        .iter()
        .any(|death| context.player_controlled.get(death.entity).is_ok());

    process_death_batch(deaths, &mut context);

    if player_died {
        transition_mission_outcome(
            MissionOutcome::Defeat,
            &mut outcome,
            &mut clock,
            &mut mission_ended,
        );
    }
}

#[derive(Debug, Clone, Copy)]
struct OrderedDeath {
    death: UnitDied,
    depth: usize,
    stable_id: Option<&'static str>,
}

fn process_death_batch(deaths: Vec<UnitDied>, context: &mut SuccessionContext) {
    let mut ordered: Vec<_> = deaths
        .into_iter()
        .map(|death| OrderedDeath {
            depth: command_depth(&context.forest, death.entity),
            stable_id: context
                .identities
                .get(death.entity)
                .ok()
                .map(|identity| identity.id.0),
            death,
        })
        .collect();
    ordered.sort_by(compare_deaths);

    for ordered_death in ordered {
        process_one_death(ordered_death.death, context);
    }
}

fn compare_deaths(a: &OrderedDeath, b: &OrderedDeath) -> Ordering {
    b.depth
        .cmp(&a.depth)
        .then_with(|| match (a.stable_id, b.stable_id) {
            (Some(a), Some(b)) => a.cmp(b),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => a.death.entity.to_bits().cmp(&b.death.entity.to_bits()),
        })
        .then_with(|| a.death.entity.to_bits().cmp(&b.death.entity.to_bits()))
}

fn command_depth(forest: &CommandForest, entity: Entity) -> usize {
    let mut depth = 0;
    let mut current = entity;
    let mut visited = HashSet::new();
    while let Some(parent) = forest.superior_of(current) {
        if !visited.insert(current) {
            break;
        }
        depth += 1;
        current = parent;
    }
    depth
}

fn process_one_death(death: UnitDied, context: &mut SuccessionContext) {
    let membership = context.memberships.get(death.entity).ok().copied();
    let was_player_controlled = context.player_controlled.get(death.entity).is_ok();
    if was_player_controlled && context.selected.entity == Some(death.entity) {
        context.selected.entity = None;
    }

    let mut squad_transition = None;
    let successor = membership.and_then(|membership| {
        let Ok(squad) = context.squads.get(membership.squad) else {
            warn!(
                deceased = ?death.entity,
                squad = ?membership.squad,
                "command succession found invalid squad membership"
            );
            return None;
        };
        if squad.current_leader != Some(death.entity) {
            return None;
        }

        let successor = squad.next_successor(|candidate| {
            context
                .eligible_units
                .get(candidate)
                .is_ok_and(|allegiance| allegiance.side == squad.side)
                && context.forest.superior_of(candidate) == Some(death.entity)
        });
        squad_transition = Some((membership.squad, successor));
        successor
    });

    let succession_outcome = match context.forest.succeed(death.entity, successor) {
        Ok(outcome) => outcome,
        Err(error) => {
            warn!(
                deceased = ?death.entity,
                ?successor,
                ?error,
                "command succession forest rewrite failed"
            );
            return;
        }
    };

    if let Some(membership) = membership {
        match context.squads.get_mut(membership.squad) {
            Ok(mut squad) => {
                if let Some((_, successor)) = squad_transition {
                    squad.current_leader = successor;
                }
                squad.revision += 1;
                if squad_transition.is_some() {
                    context.command_succeeded.write(CommandSucceeded {
                        squad: membership.squad,
                        deceased: death.entity,
                        successor,
                        tick: death.tick,
                        squad_revision: squad.revision,
                    });
                }
            }
            Err(error) => warn!(
                deceased = ?death.entity,
                squad = ?membership.squad,
                ?error,
                "successful forest rewrite could not update squad"
            ),
        }
    }

    context.structure_changed.write(CommandStructureChanged {
        tick: death.tick,
        outcome: succession_outcome,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::units::{Dead, Rank, Role, Side};
    use crate::gameplay::command::{CommandForest, CommandPlugin, UnitId};
    use crate::gameplay::objectives::ObjectivesPlugin;
    use crate::gameplay::simulation::SimulationPlugin;
    use crate::player::control::PlayerControl;
    use bevy::ecs::system::RunSystemOnce;

    fn init_world() -> World {
        let mut world = World::new();
        world.init_resource::<CommandForest>();
        world.init_resource::<SelectedUnit>();
        world.init_resource::<Messages<UnitDied>>();
        world.init_resource::<Messages<CommandStructureChanged>>();
        world.init_resource::<Messages<CommandSucceeded>>();
        world
    }

    fn spawn_unit(world: &mut World, id: &'static str, side: Side, alive: bool) -> Entity {
        let mut unit = world.spawn((
            Soldier {
                rank: Rank::Private,
                role: Role::Rifleman,
            },
            Allegiance { side },
            UnitIdentity { id: UnitId(id) },
        ));
        if alive {
            unit.insert(Alive);
        } else {
            unit.insert(Dead);
        }
        unit.id()
    }

    fn spawn_squad(world: &mut World, side: Side, members: &[Entity]) -> Entity {
        let squad = world
            .spawn(Squad {
                id: crate::gameplay::squads::SquadId("test"),
                label: "Test",
                side,
                members: members.to_vec(),
                current_leader: members.first().copied(),
                revision: 0,
            })
            .id();
        for (index, member) in members.iter().copied().enumerate() {
            world.entity_mut(member).insert(MemberOfSquad {
                squad,
                roster_index: index as u16,
            });
        }
        squad
    }

    fn install_star_forest(world: &mut World, leader: Entity, members: &[Entity]) {
        let mut forest = CommandForest::default();
        forest.ensure_node(leader);
        for member in members.iter().copied().filter(|member| *member != leader) {
            forest.set_superior(member, Some(leader)).unwrap();
        }
        world.insert_resource(forest);
    }

    fn send_deaths(world: &mut World, deaths: impl IntoIterator<Item = UnitDied>) {
        for death in deaths {
            world.write_message(death);
        }
        world.run_system_once(process_combat_deaths).unwrap();
        world.flush();
    }

    fn combat_death(entity: Entity, tick: u64) -> UnitDied {
        UnitDied {
            entity,
            tick,
            cause: UnitDeathCause::Combat { attacker: entity },
        }
    }

    #[derive(Resource)]
    struct DebugDeathTarget(Entity);

    fn emit_debug_death(
        mut commands: Commands,
        target: Res<DebugDeathTarget>,
        clock: Res<SimulationClock>,
    ) {
        crate::gameplay::lifecycle::kill_unit(
            &mut commands,
            target.0,
            clock.tick,
            UnitDeathCause::Debug,
        );
    }

    #[test]
    fn leader_death_promotes_roster_successor_and_emits_messages() {
        let mut world = init_world();
        let leader = spawn_unit(&mut world, "leader", Side::Blue, false);
        let successor = spawn_unit(&mut world, "successor", Side::Blue, true);
        let member = spawn_unit(&mut world, "member", Side::Blue, true);
        let player_unit = spawn_unit(&mut world, "player", Side::Blue, true);
        world.entity_mut(player_unit).insert(PlayerControlledUnit);
        let squad = spawn_squad(&mut world, Side::Blue, &[leader, successor, member]);
        install_star_forest(&mut world, leader, &[leader, successor, member]);

        send_deaths(&mut world, [combat_death(leader, 12)]);

        let squad_state = world.get::<Squad>(squad).unwrap();
        assert_eq!(squad_state.current_leader, Some(successor));
        assert_eq!(squad_state.revision, 1);
        let forest = world.resource::<CommandForest>();
        assert_eq!(forest.superior_of(successor), None);
        assert_eq!(forest.superior_of(member), Some(successor));
        assert!(world.get::<PlayerControlledUnit>(player_unit).is_some());
        assert!(world.get::<PlayerControlledUnit>(successor).is_none());
        assert_eq!(
            world.resource::<Messages<CommandStructureChanged>>().len(),
            1
        );
        let successions: Vec<_> = world
            .resource_mut::<Messages<CommandSucceeded>>()
            .drain()
            .collect();
        assert_eq!(
            successions,
            vec![CommandSucceeded {
                squad,
                deceased: leader,
                successor: Some(successor),
                tick: 12,
                squad_revision: 1,
            }]
        );
    }

    #[test]
    fn middle_node_leader_death_places_successor_under_old_superior() {
        let mut world = init_world();
        let parent = spawn_unit(&mut world, "parent", Side::Blue, true);
        let leader = spawn_unit(&mut world, "leader", Side::Blue, false);
        let successor = spawn_unit(&mut world, "successor", Side::Blue, true);
        spawn_squad(&mut world, Side::Blue, &[leader, successor]);
        install_star_forest(&mut world, leader, &[leader, successor]);
        world
            .resource_mut::<CommandForest>()
            .set_superior(leader, Some(parent))
            .unwrap();

        send_deaths(&mut world, [combat_death(leader, 13)]);

        assert_eq!(
            world.resource::<CommandForest>().superior_of(successor),
            Some(parent)
        );
    }

    #[test]
    fn simultaneous_leader_and_first_successor_deaths_promote_next_member() {
        let mut world = init_world();
        let leader = spawn_unit(&mut world, "leader", Side::Blue, false);
        let first = spawn_unit(&mut world, "first", Side::Blue, false);
        let second = spawn_unit(&mut world, "second", Side::Blue, true);
        let squad = spawn_squad(&mut world, Side::Blue, &[leader, first, second]);
        install_star_forest(&mut world, leader, &[leader, first, second]);

        send_deaths(
            &mut world,
            [combat_death(leader, 20), combat_death(first, 20)],
        );

        let squad_state = world.get::<Squad>(squad).unwrap();
        assert_eq!(squad_state.current_leader, Some(second));
        assert_eq!(squad_state.revision, 2);
        let forest = world.resource::<CommandForest>();
        assert_eq!(forest.superior_of(second), None);
        assert!(forest.subordinates_of(leader).is_empty());
        assert!(forest.subordinates_of(first).is_empty());
    }

    #[test]
    fn eligibility_requires_side_life_and_direct_child_topology() {
        let mut world = init_world();
        let leader = spawn_unit(&mut world, "leader", Side::Blue, false);
        let dead = spawn_unit(&mut world, "dead", Side::Blue, false);
        let wrong_side = spawn_unit(&mut world, "wrong", Side::Red, true);
        let incapable = world
            .spawn((
                Alive,
                Allegiance { side: Side::Blue },
                UnitIdentity {
                    id: UnitId("incapable"),
                },
            ))
            .id();
        let detached = spawn_unit(&mut world, "detached", Side::Blue, true);
        let eligible = spawn_unit(&mut world, "eligible", Side::Blue, true);
        let squad = spawn_squad(
            &mut world,
            Side::Blue,
            &[leader, dead, wrong_side, incapable, detached, eligible],
        );
        install_star_forest(
            &mut world,
            leader,
            &[leader, dead, wrong_side, incapable, detached, eligible],
        );
        world
            .resource_mut::<CommandForest>()
            .set_superior(detached, None)
            .unwrap();

        send_deaths(&mut world, [combat_death(leader, 3)]);

        assert_eq!(
            world.get::<Squad>(squad).unwrap().current_leader,
            Some(eligible)
        );
    }

    #[test]
    fn leader_without_eligible_successor_leaves_squad_unled_and_orphans_children() {
        let mut world = init_world();
        let leader = spawn_unit(&mut world, "leader", Side::Blue, false);
        let dead_member = spawn_unit(&mut world, "dead", Side::Blue, false);
        let squad = spawn_squad(&mut world, Side::Blue, &[leader, dead_member]);
        install_star_forest(&mut world, leader, &[leader, dead_member]);

        send_deaths(&mut world, [combat_death(leader, 4)]);

        let squad_state = world.get::<Squad>(squad).unwrap();
        assert_eq!(squad_state.current_leader, None);
        assert_eq!(squad_state.revision, 1);
        assert_eq!(
            world.resource::<CommandForest>().superior_of(dead_member),
            None
        );
    }

    #[test]
    fn nonleader_death_removes_node_without_changing_leader() {
        let mut world = init_world();
        let leader = spawn_unit(&mut world, "leader", Side::Blue, true);
        let member = spawn_unit(&mut world, "member", Side::Blue, false);
        let squad = spawn_squad(&mut world, Side::Blue, &[leader, member]);
        install_star_forest(&mut world, leader, &[leader, member]);

        send_deaths(&mut world, [combat_death(member, 4)]);

        let squad_state = world.get::<Squad>(squad).unwrap();
        assert_eq!(squad_state.current_leader, Some(leader));
        assert_eq!(squad_state.revision, 1);
        assert!(world.resource::<Messages<CommandSucceeded>>().is_empty());
        assert!(
            world
                .resource::<CommandForest>()
                .subordinates_of(leader)
                .is_empty()
        );
    }

    #[test]
    fn failed_forest_rewrite_does_not_mutate_squad_or_emit_messages() {
        let mut world = init_world();
        let leader = spawn_unit(&mut world, "leader", Side::Blue, false);
        let successor = spawn_unit(&mut world, "successor", Side::Blue, true);
        let squad = spawn_squad(&mut world, Side::Blue, &[leader, successor]);
        world.entity_mut(leader).insert(PlayerControlledUnit);
        world.resource_mut::<SelectedUnit>().entity = Some(leader);
        world.resource_mut::<CommandForest>().ensure_node(successor);

        send_deaths(&mut world, [combat_death(leader, 8)]);

        let squad_state = world.get::<Squad>(squad).unwrap();
        assert_eq!(squad_state.current_leader, Some(leader));
        assert_eq!(squad_state.revision, 0);
        assert_eq!(world.resource::<SelectedUnit>().entity, None);
        assert!(
            world
                .resource::<Messages<CommandStructureChanged>>()
                .is_empty()
        );
    }

    #[test]
    fn equal_depth_deaths_use_stable_unit_id_message_order() {
        let mut world = init_world();
        let zulu = spawn_unit(&mut world, "zulu", Side::Blue, false);
        let alpha = spawn_unit(&mut world, "alpha", Side::Blue, false);
        world.resource_mut::<CommandForest>().ensure_node(zulu);
        world.resource_mut::<CommandForest>().ensure_node(alpha);

        send_deaths(&mut world, [combat_death(zulu, 5), combat_death(alpha, 5)]);

        let changes: Vec<_> = world
            .resource_mut::<Messages<CommandStructureChanged>>()
            .drain()
            .collect();
        assert_eq!(changes[0].outcome.deceased, alpha);
        assert_eq!(changes[1].outcome.deceased, zulu);
    }

    #[test]
    fn command_plugin_processes_combat_death_in_fixed_cleanup() {
        let mut app = App::new();
        app.add_plugins((SimulationPlugin, CommandPlugin, ObjectivesPlugin));
        app.init_resource::<SelectedUnit>();
        app.init_resource::<PlayerControl>();
        app.init_resource::<Messages<UnitDied>>();
        app.world_mut()
            .insert_resource(State::new(GameState::MissionScreen));
        let leader = spawn_unit(app.world_mut(), "leader", Side::Blue, false);
        let successor = spawn_unit(app.world_mut(), "successor", Side::Blue, true);
        app.world_mut()
            .entity_mut(leader)
            .insert(PlayerControlledUnit);
        let squad = spawn_squad(app.world_mut(), Side::Blue, &[leader, successor]);
        install_star_forest(app.world_mut(), leader, &[leader, successor]);
        app.world_mut().write_message(combat_death(leader, 1));

        app.world_mut().run_schedule(FixedUpdate);

        assert_eq!(
            app.world().get::<Squad>(squad).unwrap().current_leader,
            Some(successor)
        );
        assert_eq!(
            app.world()
                .resource::<CommandForest>()
                .superior_of(successor),
            None
        );
        assert_eq!(
            *app.world().resource::<MissionOutcome>(),
            MissionOutcome::Defeat
        );
    }

    #[test]
    fn command_plugin_handles_flushed_debug_death_in_update_while_paused() {
        let mut app = App::new();
        app.add_plugins(CommandPlugin);
        app.init_resource::<SelectedUnit>();
        app.init_resource::<Messages<UnitDied>>();
        app.init_resource::<Messages<MissionEnded>>();
        app.insert_resource(MissionOutcome::InProgress);
        app.insert_resource(SimulationClock {
            paused: true,
            tick: 11,
            ..default()
        });
        app.world_mut()
            .insert_resource(State::new(GameState::MissionScreen));
        let leader = spawn_unit(app.world_mut(), "leader", Side::Blue, true);
        let successor = spawn_unit(app.world_mut(), "successor", Side::Blue, true);
        spawn_squad(app.world_mut(), Side::Blue, &[leader, successor]);
        install_star_forest(app.world_mut(), leader, &[leader, successor]);
        app.world_mut()
            .entity_mut(leader)
            .insert(PlayerControlledUnit);
        app.world_mut().resource_mut::<SelectedUnit>().entity = Some(leader);
        app.insert_resource(DebugDeathTarget(leader));
        app.add_systems(
            Update,
            (emit_debug_death, ApplyDeferred)
                .chain()
                .in_set(DebugPowersSet::DeathCommands),
        );

        app.update();

        assert_eq!(
            *app.world().resource::<MissionOutcome>(),
            MissionOutcome::Defeat
        );
        assert!(app.world().resource::<SimulationClock>().paused);
        assert_eq!(app.world().resource::<SelectedUnit>().entity, None);
        assert_eq!(
            app.world()
                .get::<Squad>(app.world().get::<MemberOfSquad>(leader).unwrap().squad)
                .unwrap()
                .current_leader,
            Some(successor)
        );
        assert!(app.world().get::<PlayerControlledUnit>(leader).is_some());
        assert!(app.world().get::<PlayerControlledUnit>(successor).is_none());
        assert_eq!(app.world().resource::<Messages<MissionEnded>>().len(), 1);
    }

    #[test]
    fn paused_debug_player_death_defeats_without_transferring_control() {
        let mut world = init_world();
        world.insert_resource(MissionOutcome::InProgress);
        world.insert_resource(SimulationClock {
            paused: true,
            tick: 9,
            ..default()
        });
        world.init_resource::<Messages<MissionEnded>>();
        let leader = spawn_unit(&mut world, "leader", Side::Blue, false);
        let successor = spawn_unit(&mut world, "successor", Side::Blue, true);
        spawn_squad(&mut world, Side::Blue, &[leader, successor]);
        install_star_forest(&mut world, leader, &[leader, successor]);
        world.entity_mut(leader).insert(PlayerControlledUnit);
        world.resource_mut::<SelectedUnit>().entity = Some(leader);
        world.write_message(UnitDied {
            entity: leader,
            tick: 9,
            cause: UnitDeathCause::Debug,
        });

        world.run_system_once(process_debug_deaths).unwrap();
        world.flush();

        assert_eq!(*world.resource::<MissionOutcome>(), MissionOutcome::Defeat);
        assert!(world.resource::<SimulationClock>().paused);
        assert!(world.get::<PlayerControlledUnit>(leader).is_some());
        assert!(world.get::<PlayerControlledUnit>(successor).is_none());
        assert_eq!(world.resource::<SelectedUnit>().entity, None);
        assert_eq!(world.resource::<Messages<MissionEnded>>().len(), 1);
    }
}
