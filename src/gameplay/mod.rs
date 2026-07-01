#![doc = include_str!("../../docs/gameplay.md")]

pub mod combat;
pub mod command;
pub mod comms;
pub mod components;
pub mod diagnostics;
pub mod lifecycle;
pub mod map;
pub mod measurements;
pub mod rendering;
pub mod simulation;
pub mod terrain;

use bevy::app::{PluginGroup, PluginGroupBuilder};
use combat::CombatPlugin;
use command::CommandPlugin;
use comms::CommsPlugin;
use diagnostics::GameplayDiagnosticsPlugin;
use lifecycle::UnitLifecyclePlugin;
use rendering::GameplayRenderingPlugin;
use simulation::SimulationPlugin;

pub struct GameplayPlugins;

impl PluginGroup for GameplayPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(SimulationPlugin)
            .add(UnitLifecyclePlugin)
            .add(CommandPlugin)
            .add(CommsPlugin)
            .add(CombatPlugin)
            .add(GameplayDiagnosticsPlugin)
            .add(GameplayRenderingPlugin)
    }
}
