#![doc = include_str!("../../docs/player.md")]

pub mod control;
pub mod knowledge;
pub mod mission_placement;
pub mod selection;

use bevy::app::{PluginGroup, PluginGroupBuilder};
use control::PlayerControlPlugin;
use knowledge::PlayerKnowledgePlugin;
use mission_placement::MissionPlacementPlugin;
use selection::SelectionPlugin;

pub struct PlayerPlugins;

impl PluginGroup for PlayerPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(PlayerControlPlugin)
            .add(PlayerKnowledgePlugin)
            .add(MissionPlacementPlugin)
            .add(SelectionPlugin)
    }
}
