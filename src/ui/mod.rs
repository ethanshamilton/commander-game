pub mod active_action;
pub mod widgets;

use active_action::ActiveActionPlugin;
use bevy::prelude::*;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ActiveActionPlugin);
    }
}
