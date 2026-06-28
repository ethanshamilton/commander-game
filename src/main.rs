mod actions;
mod ai;
mod gameplay;
mod maps;
mod missions;
mod player;
mod screens;
mod units;

use ai::AiPlugins;
use bevy::gilrs::GilrsPlugin;
use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode, WindowPlugin};
use gameplay::GameplayPlugins;
use player::PlayerPlugins;
use screens::ScreensPlugins;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
                        ..default()
                    }),
                    ..default()
                })
                .disable::<GilrsPlugin>(),
        )
        .add_plugins(GameplayPlugins)
        .add_plugins(AiPlugins)
        .add_plugins(PlayerPlugins)
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
