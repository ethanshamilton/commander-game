#![doc = include_str!("../../docs/ai.md")]

pub mod htn;
pub mod perception;

use bevy::app::{PluginGroup, PluginGroupBuilder};
use perception::PerceptionPlugin;

pub struct AiPlugins;

impl PluginGroup for AiPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>().add(PerceptionPlugin)
    }
}
