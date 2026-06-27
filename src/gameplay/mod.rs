pub mod components;
pub mod rendering;
pub mod simulation;

use bevy::app::{PluginGroup, PluginGroupBuilder};
use rendering::GameplayRenderingPlugin;
use simulation::SimulationPlugin;

pub struct GameplayPlugins;

impl PluginGroup for GameplayPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(SimulationPlugin)
            .add(GameplayRenderingPlugin)
    }
}
