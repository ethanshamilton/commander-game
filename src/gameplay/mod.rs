#![doc = include_str!("../../docs/gameplay.md")]

pub mod audio;
pub mod combat;
pub mod command;
pub mod comms;
pub mod debug_powers;
pub mod diagnostics;
pub mod lifecycle;
pub mod map;
pub mod measurements;
pub mod objectives;
pub mod orders;
pub mod packets;
pub mod rendering;
pub mod simulation;
pub mod spatial;
pub mod terrain;

use audio::GameplayAudioPlugin;
use bevy::app::{PluginGroup, PluginGroupBuilder};
use combat::CombatPlugin;
use command::CommandPlugin;
use comms::CommsPlugin;
use debug_powers::DebugPowersPlugin;
use diagnostics::GameplayDiagnosticsPlugin;
use lifecycle::UnitLifecyclePlugin;
use objectives::ObjectivesPlugin;
use packets::PacketsPlugin;
use rendering::GameplayRenderingPlugin;
use simulation::SimulationPlugin;

pub struct GameplayPlugins;

impl PluginGroup for GameplayPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(SimulationPlugin)
            .add(UnitLifecyclePlugin)
            .add(DebugPowersPlugin)
            .add(CommandPlugin)
            .add(CommsPlugin)
            .add(PacketsPlugin)
            .add(CombatPlugin)
            .add(ObjectivesPlugin)
            .add(GameplayDiagnosticsPlugin)
            .add(GameplayRenderingPlugin)
            .add(GameplayAudioPlugin)
    }
}
