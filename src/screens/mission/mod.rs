#![doc = include_str!("../../../docs/screens/mission.md")]

mod hud;
mod layout;
mod menu;
mod performance_overlay;
mod plan_panel;
mod unit_ai_debug;
mod unit_panel;

use crate::GameState;
use crate::gameplay::mission_runtime::{cleanup_mission_runtime, setup_selected_mission};
use bevy::prelude::*;

#[derive(Component)]
pub(super) struct MissionScreenRoot;

pub struct MissionScreenPlugin;

impl Plugin for MissionScreenPlugin {
    fn build(&self, app: &mut App) {
        menu::register(app);
        plan_panel::register(app);
        unit_panel::register(app);
        unit_ai_debug::register(app);
        performance_overlay::register(app);
        hud::register(app);

        app.add_systems(
            OnEnter(GameState::MissionScreen),
            (layout::setup_mission_ui, setup_selected_mission),
        )
        .add_systems(
            OnExit(GameState::MissionScreen),
            (cleanup_mission_ui, cleanup_mission_runtime),
        );
    }
}

fn cleanup_mission_ui(mut commands: Commands, roots: Query<Entity, With<MissionScreenRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}
