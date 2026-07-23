use crate::actors::spawning::{SoldierSpawn, spawn_soldier};
use crate::actors::units::{Rank, Side};
use crate::gameplay::command::{CommandForest, UnitIdentity};
use crate::gameplay::command_plans::CommandPlanIdAllocator;
use crate::gameplay::lifecycle::MissionScoped;
use crate::gameplay::map::BattlefieldMap;
use crate::gameplay::objectives::{MissionObjectiveSet, MissionOutcome};
use crate::gameplay::packets::PacketIdAllocator;
use crate::gameplay::simulation::SimulationClock;
use crate::gameplay::squads::{MemberOfSquad, Squad};
use crate::missions::{MissionDefinition, SelectedMission};
use crate::player::knowledge::{PlayerControlledUnit, PlayerTacticalKnowledge};
use crate::player::selection::SelectedUnit;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

pub fn setup_selected_mission(mut commands: Commands, selected: Option<Res<SelectedMission>>) {
    let Some(selected) = selected else {
        panic!("MISSION DOES NOT EXIST: MissionScreen entered without SelectedMission");
    };

    instantiate_mission(&mut commands, selected.mission);
}

/// Instantiate a compiled mission definition into runtime resources and entities.
pub fn instantiate_mission(commands: &mut Commands, mission: &MissionDefinition) {
    info!(
        "Spawning mission: {} ({}) on map: {}",
        mission.name, mission.id, mission.map.name
    );
    commands.insert_resource(BattlefieldMap::from_definition(mission.map));
    commands.insert_resource(SimulationClock::default());
    commands.insert_resource(PlayerTacticalKnowledge::default());
    commands.insert_resource(PacketIdAllocator::default());
    commands.insert_resource(SelectedUnit::default());
    commands.insert_resource(MissionObjectiveSet::from_slices(
        mission.victory_conditions,
        mission.defeat_conditions,
    ));
    commands.insert_resource(MissionOutcome::InProgress);

    let mut entities_by_unit_id = HashMap::new();
    let mut side_by_entity = HashMap::new();

    for unit in mission.units {
        let entity = spawn_soldier(
            commands,
            SoldierSpawn {
                rank: unit.rank,
                role: unit.role,
                side: unit.side,
                position_m: Vec2::new(unit.position_meters[0], unit.position_meters[1]),
                heading_radians: unit.heading_radians,
            },
        );

        commands.entity(entity).insert(UnitIdentity { id: unit.id });
        entities_by_unit_id.insert(unit.id, entity);
        side_by_entity.insert(entity, unit.side);

        if unit.side == Side::Blue && matches!(unit.rank, Rank::Sergeant) {
            commands.entity(entity).insert(PlayerControlledUnit);
        }
    }

    let mut command_forest = CommandForest::from_assignments(
        mission.command_assignments,
        &entities_by_unit_id,
        |entity| side_by_entity.get(&entity).copied(),
    );
    for &entity in entities_by_unit_id.values() {
        command_forest.ensure_node(entity);
    }

    let mut assigned_members = HashSet::new();
    let mut squad_ids = HashSet::new();
    for definition in mission.squads {
        assert!(
            squad_ids.insert(definition.id),
            "duplicate squad id {:?}",
            definition.id
        );
        assert!(
            !definition.members.is_empty(),
            "squad {:?} has an empty roster",
            definition.id
        );
        assert!(
            definition.members.len() <= u16::MAX as usize,
            "squad {:?} roster is too large",
            definition.id
        );

        let members: Vec<_> = definition
            .members
            .iter()
            .map(|unit_id| {
                let entity = *entities_by_unit_id.get(unit_id).unwrap_or_else(|| {
                    panic!(
                        "squad {:?} references unknown unit {:?}",
                        definition.id, unit_id
                    )
                });
                assert!(
                    assigned_members.insert(entity),
                    "unit {:?} belongs to more than one squad",
                    unit_id
                );
                entity
            })
            .collect();
        let leader = members[0];
        let side = side_by_entity[&leader];
        assert!(
            members
                .iter()
                .all(|member| side_by_entity.get(member) == Some(&side)),
            "squad {:?} contains units from different sides",
            definition.id
        );

        for &member in &members[1..] {
            command_forest
                .set_superior(member, Some(leader))
                .unwrap_or_else(|error| {
                    panic!(
                        "invalid internal command link in squad {:?}: {error}",
                        definition.id
                    )
                });
        }

        let squad = commands
            .spawn((
                MissionScoped,
                Squad {
                    id: definition.id,
                    label: definition.label,
                    side,
                    members: members.clone(),
                    current_leader: Some(leader),
                    revision: 0,
                },
            ))
            .id();
        for (roster_index, member) in members.into_iter().enumerate() {
            commands.entity(member).insert(MemberOfSquad {
                squad,
                roster_index: roster_index as u16,
            });
        }
    }

    commands.insert_resource(command_forest);
}

pub fn cleanup_mission_runtime(
    mut commands: Commands,
    mission_entities: Query<Entity, With<MissionScoped>>,
    mut mission_ids: ResMut<CommandPlanIdAllocator>,
) {
    for entity in &mission_entities {
        commands.entity(entity).despawn();
    }

    mission_ids.reset();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::command::{UnitId, UnitIdentity};
    use crate::gameplay::squads::SquadId;
    use crate::missions::SINGLE_SQUAD_COMMAND_TUTORIAL_SCENARIO;
    use bevy::ecs::world::CommandQueue;

    #[test]
    fn mission_instantiation_builds_ordered_squads_and_internal_command_links() {
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        instantiate_mission(&mut commands, &SINGLE_SQUAD_COMMAND_TUTORIAL_SCENARIO);
        queue.apply(&mut world);

        let identities: HashMap<_, _> = world
            .query::<(Entity, &UnitIdentity)>()
            .iter(&world)
            .map(|(entity, identity)| (identity.id, entity))
            .collect();
        let blue_leader = identities[&UnitId("blue_sergeant")];
        let expected_roster = [
            blue_leader,
            identities[&UnitId("blue_rifleman_1")],
            identities[&UnitId("blue_rifleman_2")],
            identities[&UnitId("blue_rifleman_3")],
        ];
        let blue_squad = world
            .query::<&Squad>()
            .iter(&world)
            .find(|squad| squad.id == SquadId("blue_squad"))
            .unwrap();

        assert_eq!(blue_squad.members, expected_roster);
        assert_eq!(blue_squad.current_leader, Some(blue_leader));

        let forest = world.resource::<CommandForest>();
        for &member in &expected_roster[1..] {
            assert_eq!(forest.superior_of(member), Some(blue_leader));
        }
        for (index, &member) in expected_roster.iter().enumerate() {
            let membership = world.get::<MemberOfSquad>(member).unwrap();
            assert_eq!(membership.roster_index, index as u16);
        }
    }
}
