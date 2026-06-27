use crate::units::Side;
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

impl Default for PlayerControl {
    fn default() -> Self {
        Self { side: Side::Blue }
    }
}
