mod actions;
mod screens;
mod units;

use bevy::prelude::*;
use screens::ScreensPlugins;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ScreensPlugins)
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
