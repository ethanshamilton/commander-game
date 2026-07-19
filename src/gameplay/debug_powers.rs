use crate::GameState;
use crate::actors::units::{Health, Soldier};
use crate::gameplay::lifecycle::kill_unit;
use crate::player::selection::SelectedUnit;
use bevy::prelude::*;

pub struct DebugPowersPlugin;

impl Plugin for DebugPowersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            debug_kill_selected_unit.run_if(in_state(GameState::MissionScreen)),
        );
    }
}

fn debug_kill_selected_unit(
    keyboard: Res<ButtonInput<KeyCode>>,
    selected: Res<SelectedUnit>,
    mut commands: Commands,
    mut health: Query<&mut Health, With<Soldier>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyK) {
        return;
    }

    let Some(entity) = selected.entity else {
        return;
    };

    if let Ok(mut health) = health.get_mut(entity) {
        health.current = 0;
    }

    kill_unit(&mut commands, entity);
    info!("Debug killed selected unit: {entity:?}");
}
