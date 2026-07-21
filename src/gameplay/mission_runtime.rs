use crate::actors::spawning::{SoldierSpawn, spawn_soldier};
use crate::actors::units::{Rank, Side};
use crate::gameplay::command::{CommandForest, UnitIdentity};
use crate::gameplay::command_plans::CommandPlanIdAllocator;
use crate::gameplay::lifecycle::MissionScoped;
use crate::gameplay::map::BattlefieldMap;
use crate::gameplay::objectives::{MissionObjectiveSet, MissionOutcome};
use crate::gameplay::packets::PacketIdAllocator;
use crate::gameplay::simulation::SimulationClock;
use crate::missions::{MissionDefinition, SelectedMission};
use crate::player::knowledge::{PlayerControlledUnit, PlayerTacticalKnowledge};
use crate::player::selection::SelectedUnit;
use bevy::prelude::*;
use std::collections::HashMap;

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

    commands.insert_resource(CommandForest::from_assignments(
        mission.command_assignments,
        &entities_by_unit_id,
        |entity| side_by_entity.get(&entity).copied(),
    ));
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
