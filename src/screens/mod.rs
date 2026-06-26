pub mod main_menu;
pub mod mission;
pub mod settings;

use bevy::app::{PluginGroup, PluginGroupBuilder};
use bevy::prelude::*;

use main_menu::MainMenuScreenPlugin;
use mission::MissionScreenPlugin;
use settings::SettingsScreenPlugin;

pub struct ScreensPluginGroup;

impl PluginGroup for ScreensPluginGroup {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(ScreenInfrastructurePlugin)
            .add(MainMenuScreenPlugin)
            .add(MissionScreenPlugin)
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
    commands.insert_resource(ClearColor(Color::srgb(0.1, 0.12, 0.14)));
}

// Re-export screen plugin group for main.rs.
pub use ScreensPluginGroup as ScreensPlugins;
