pub mod control;
pub mod selection;

use bevy::app::{PluginGroup, PluginGroupBuilder};
use control::PlayerControlPlugin;
use selection::SelectionPlugin;

pub struct PlayerPlugins;

impl PluginGroup for PlayerPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(PlayerControlPlugin)
            .add(SelectionPlugin)
    }
}
