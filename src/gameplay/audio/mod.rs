#![doc = include_str!("../../../docs/gameplay/audio.md")]

mod combat;

use bevy::prelude::*;
use combat::CombatAudioPlugin;

pub struct GameplayAudioPlugin;

impl Plugin for GameplayAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CombatAudioPlugin);
    }
}
