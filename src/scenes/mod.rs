pub mod main_menu;
pub mod mission;
pub mod settings;

use bevy::app::{PluginGroup, PluginGroupBuilder};
use bevy::prelude::*;

use main_menu::MainMenuScenePlugin;
use mission::MissionScenePlugin;
use settings::SettingsScenePlugin;

pub struct ScenesPluginGroup;

impl PluginGroup for ScenesPluginGroup {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(SceneInfrastructurePlugin)
            .add(MainMenuScenePlugin)
            .add(MissionScenePlugin)
            .add(SettingsScenePlugin)
    }
}

struct SceneInfrastructurePlugin;

impl Plugin for SceneInfrastructurePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_scene_camera);
    }
}

fn setup_scene_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.insert_resource(ClearColor(Color::srgb(0.1, 0.12, 0.14)));
}

// Re-export scene plugin group for main.rs.
pub use ScenesPluginGroup as ScenesPlugins;
