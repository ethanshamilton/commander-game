pub mod components;
pub mod diagnostics;
pub mod measurements;
pub mod rendering;
pub mod simulation;
pub mod terrain;

use bevy::app::{PluginGroup, PluginGroupBuilder};
use diagnostics::GameplayDiagnosticsPlugin;
use rendering::GameplayRenderingPlugin;
use simulation::SimulationPlugin;

pub struct GameplayPlugins;

impl PluginGroup for GameplayPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(SimulationPlugin)
            .add(GameplayDiagnosticsPlugin)
            .add(GameplayRenderingPlugin)
    }
}
