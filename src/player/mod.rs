pub mod control;
pub mod knowledge;
pub mod selection;

use bevy::app::{PluginGroup, PluginGroupBuilder};
use control::PlayerControlPlugin;
use knowledge::PlayerKnowledgePlugin;
use selection::SelectionPlugin;

pub struct PlayerPlugins;

impl PluginGroup for PlayerPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(PlayerControlPlugin)
            .add(PlayerKnowledgePlugin)
            .add(SelectionPlugin)
    }
}
