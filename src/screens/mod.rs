#![doc = include_str!("../../docs/screens.md")]

pub mod main_menu;
pub mod scenario;
pub mod scenario_brief;
pub mod scenario_select;
pub mod settings;

use bevy::app::{PluginGroup, PluginGroupBuilder};
use bevy::prelude::*;

use main_menu::MainMenuScreenPlugin;
use scenario::ScenarioScreenPlugin;
use scenario_brief::ScenarioBriefScreenPlugin;
use scenario_select::ScenarioSelectScreenPlugin;
use settings::SettingsScreenPlugin;

pub struct ScreensPluginGroup;

impl PluginGroup for ScreensPluginGroup {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(ScreenInfrastructurePlugin)
            .add(MainMenuScreenPlugin)
            .add(ScenarioSelectScreenPlugin)
            .add(ScenarioBriefScreenPlugin)
            .add(ScenarioScreenPlugin)
            .add(SettingsScreenPlugin)
    }
}

struct ScreenInfrastructurePlugin;

impl Plugin for ScreenInfrastructurePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_screen_camera);
    }
}

fn setup_screen_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.insert_resource(ClearColor(Color::BLACK));
}

// Re-export screen plugin group for main.rs.
pub use ScreensPluginGroup as ScreensPlugins;
