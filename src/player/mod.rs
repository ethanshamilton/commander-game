#![doc = include_str!("../../docs/player.md")]

pub mod control;
pub mod knowledge;
pub mod plan_placement;
pub mod selection;

use bevy::app::{PluginGroup, PluginGroupBuilder};
use control::PlayerControlPlugin;
use knowledge::PlayerKnowledgePlugin;
use plan_placement::PlanPlacementPlugin;
use selection::SelectionPlugin;

pub struct PlayerPlugins;

impl PluginGroup for PlayerPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(PlayerControlPlugin)
            .add(PlayerKnowledgePlugin)
            .add(PlanPlacementPlugin)
            .add(SelectionPlugin)
    }
}
