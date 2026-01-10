mod actions;
mod ui;
mod units;

use actions::*;
use bevy::prelude::*;
use ui::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, (setup, setup_unit_bar))
        .add_systems(Update, interaction_system)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.insert_resource(ClearColor(Color::srgb(0.1, 0.12, 0.14)));
}
