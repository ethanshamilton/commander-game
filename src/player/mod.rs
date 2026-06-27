pub mod selection;

use bevy::app::{PluginGroup, PluginGroupBuilder};
use selection::SelectionPlugin;

pub struct PlayerPlugins;

impl PluginGroup for PlayerPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>().add(SelectionPlugin)
    }
}
