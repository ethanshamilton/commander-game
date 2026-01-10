mod actions;
mod scenes;
mod units;

use actions::*;
use bevy::prelude::*;
use scenes::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<GameState>()
        .insert_resource(MenuState::new())
        .add_systems(Startup, setup)
        // Mission Screen Lifecycle
        .add_systems(
            OnEnter(GameState::MissionScreen),
            (setup, setup_sidebar, setup_unit_bar),
        )
        .add_systems(OnExit(GameState::MissionScreen), cleanup_mission_scene)
        // Mission Screen Systems
        .add_systems(
            Update,
            (interaction_system, update_menu_visibility).run_if(in_state(GameState::MissionScreen)),
        )
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.insert_resource(ClearColor(Color::srgb(0.1, 0.12, 0.14)));
}

/// Game States
#[derive(States, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    MainMenu,
    MissionScreen,
    Settings,
}
