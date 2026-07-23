#![doc = include_str!("../../docs/gameplay.md")]

pub mod audio;
pub mod combat;
pub mod command;
pub mod command_plans;
pub mod comms;
pub mod debug_powers;
pub mod diagnostics;
pub mod formations;
pub mod lifecycle;
pub mod map;
pub mod measurements;
pub mod mission_runtime;
pub mod objectives;
pub mod orders;
pub mod packets;
pub mod rendering;
pub mod simulation;
pub mod spatial;
pub mod spatial_index;
pub mod squads;
pub mod terrain;

use audio::GameplayAudioPlugin;
use bevy::app::{PluginGroup, PluginGroupBuilder};
use combat::CombatPlugin;
use command::CommandPlugin;
use command_plans::CommandPlansPlugin;
use comms::CommsPlugin;
use debug_powers::DebugPowersPlugin;
use diagnostics::GameplayDiagnosticsPlugin;
use lifecycle::UnitLifecyclePlugin;
use objectives::ObjectivesPlugin;
use packets::PacketsPlugin;
use rendering::GameplayRenderingPlugin;
use simulation::SimulationPlugin;
use spatial_index::SpatialIndexPlugin;

pub struct GameplayPlugins;

impl PluginGroup for GameplayPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(SimulationPlugin)
            .add(SpatialIndexPlugin)
            .add(UnitLifecyclePlugin)
            .add(CommandPlansPlugin)
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
