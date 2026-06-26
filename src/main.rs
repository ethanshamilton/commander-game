mod actions;
mod scenes;
mod units;

use bevy::prelude::*;
use scenes::ScenesPlugins;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ScenesPlugins)
        .init_state::<GameState>()
        .run();
}

/// Game States
#[derive(States, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    MainMenu,
    #[default]
    MissionScreen,
    Settings,
}
