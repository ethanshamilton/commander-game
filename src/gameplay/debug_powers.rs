use crate::GameState;
use crate::actors::units::{Alive, Health, Soldier};
use crate::gameplay::lifecycle::{UnitDeathCause, kill_unit};
use crate::gameplay::simulation::SimulationClock;
use crate::input::{ActionState, GameAction};
use crate::player::selection::SelectedUnit;
use bevy::prelude::*;

pub struct DebugPowersPlugin;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DebugPowersSet {
    DeathCommands,
}

impl Plugin for DebugPowersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (debug_kill_selected_unit, ApplyDeferred)
                .chain()
                .in_set(DebugPowersSet::DeathCommands)
                .run_if(in_state(GameState::MissionScreen)),
        );
    }
}

fn debug_kill_selected_unit(
    actions: Res<ActionState>,
    selected: Res<SelectedUnit>,
    mut commands: Commands,
    clock: Res<SimulationClock>,
    mut health: Query<&mut Health, (With<Soldier>, With<Alive>)>,
) {
    if !actions.just_pressed(GameAction::DebugKillSelected) {
        return;
    }

    let Some(entity) = selected.entity else {
        return;
    };

    let Ok(mut health) = health.get_mut(entity) else {
        return;
    };
    health.current = 0;

    kill_unit(&mut commands, entity, clock.tick, UnitDeathCause::Debug);
    info!("Debug killed selected unit: {entity:?}");
}
