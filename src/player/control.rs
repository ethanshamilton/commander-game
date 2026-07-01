#![doc = include_str!("../../docs/player/control.md")]

use crate::actors::units::Side;
use bevy::prelude::*;

pub struct PlayerControlPlugin;

impl Plugin for PlayerControlPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerControl>();
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct PlayerControl {
    pub side: Side,
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct UnitIntelAccess {
    pub reveal_sensor_range: bool,
    pub reveal_contacts: bool,
}

impl Default for PlayerControl {
    fn default() -> Self {
        Self { side: Side::Blue }
    }
}
