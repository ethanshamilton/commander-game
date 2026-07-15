use crate::GameState;
use crate::gameplay::combat::ResolvedShot;
use crate::gameplay::simulation::SimulationSet;
use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;

const RIFLE_SHOT_AUDIO_PATH: &str = "audio/rifle_shot.ogg";
const RIFLE_SHOT_VOLUME: f32 = 0.65;

pub struct CombatAudioPlugin;

impl Plugin for CombatAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_combat_audio).add_systems(
            FixedUpdate,
            play_fire_sounds
                .in_set(SimulationSet::Cleanup)
                .run_if(in_state(GameState::ScenarioScreen)),
        );
    }
}

#[derive(Resource)]
struct CombatAudioAssets {
    rifle_shot: Handle<AudioSource>,
}

fn load_combat_audio(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(CombatAudioAssets {
        rifle_shot: asset_server.load(RIFLE_SHOT_AUDIO_PATH),
    });
}

fn play_fire_sounds(
    mut commands: Commands,
    audio: Res<CombatAudioAssets>,
    mut resolved_shots: MessageReader<ResolvedShot>,
) {
    for _shot in resolved_shots.read() {
        commands.spawn((
            AudioPlayer::new(audio.rifle_shot.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(RIFLE_SHOT_VOLUME)),
        ));
    }
}
