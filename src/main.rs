mod actions;
mod gameplay;
mod missions;
mod screens;
mod units;

use bevy::gilrs::GilrsPlugin;
use bevy::prelude::*;
use gameplay::GameplayPlugins;
use screens::ScreensPlugins;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.build().disable::<GilrsPlugin>())
        .add_plugins(GameplayPlugins)
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
