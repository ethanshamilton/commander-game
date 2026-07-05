#![doc = include_str!("../../docs/gameplay/lifecycle.md")]

use crate::GameState;
use crate::actors::units::{Alive, Dead, Soldier};
use crate::ai::perception::{AuditorySensor, EyeHeight, PerceptionMemory, VisualSensor};
use crate::gameplay::combat::{CombatOrder, CombatState};
use crate::gameplay::comms::{CommsLinks, VoiceComms};
use crate::gameplay::orders::{CombatOrderSource, UnitOrderSource};
use crate::gameplay::simulation::UnitOrder;
use bevy::prelude::*;

pub struct UnitLifecyclePlugin;

impl Plugin for UnitLifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            warn_invalid_life_status.run_if(in_state(GameState::MissionScreen)),
        );
    }
}

/// Transition a unit from alive actor to dead ground-truth entity.
///
/// The entity is intentionally not despawned: player knowledge may remain stale,
/// and living units may later observe the body. Death strips active capabilities
/// so the entity can no longer move, perceive, communicate, or remember/report.
pub fn kill_unit(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<Alive>()
        .insert(Dead)
        .remove::<UnitOrder>()
        .remove::<UnitOrderSource>()
        .remove::<CombatOrder>()
        .remove::<CombatOrderSource>()
        .remove::<CombatState>()
        .remove::<VisualSensor>()
        .remove::<AuditorySensor>()
        .remove::<EyeHeight>()
        .remove::<VoiceComms>()
        .remove::<CommsLinks>()
        .remove::<PerceptionMemory>();
}

fn warn_invalid_life_status(
    both: Query<Entity, (With<Soldier>, With<Alive>, With<Dead>)>,
    neither: Query<Entity, (With<Soldier>, Without<Alive>, Without<Dead>)>,
) {
    for entity in &both {
        error!("Soldier has both Alive and Dead markers: {entity:?}");
    }

    for entity in &neither {
        error!("Soldier has neither Alive nor Dead marker: {entity:?}");
    }
}
