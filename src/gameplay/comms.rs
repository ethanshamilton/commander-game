#![doc = include_str!("../../docs/gameplay/comms.md")]

use crate::gameplay::components::BattlefieldPosition;
use crate::gameplay::simulation::SimulationSet;
use crate::units::{Allegiance, Soldier};
use bevy::prelude::*;

pub const DEFAULT_VOICE_COMMS_RANGE_M: f32 = 40.0;

pub struct CommsPlugin;

impl Plugin for CommsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, update_voice_comms.in_set(SimulationSet::Comms));
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct VoiceComms {
    pub range_m: f32,
}

impl Default for VoiceComms {
    fn default() -> Self {
        Self {
            range_m: DEFAULT_VOICE_COMMS_RANGE_M,
        }
    }
}

#[derive(Component, Debug, Default)]
pub struct CommsLinks {
    pub links: Vec<CommsLink>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct CommsLink {
    pub target: Entity,
    pub kind: CommsKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommsKind {
    Voice,
}

#[derive(Debug, Clone, Copy)]
struct CommsSnapshot {
    entity: Entity,
    position_m: Vec2,
    side: crate::units::Side,
    voice: Option<VoiceComms>,
}

fn update_voice_comms(
    snapshots: Query<
        (
            Entity,
            &BattlefieldPosition,
            &Allegiance,
            Option<&VoiceComms>,
        ),
        With<Soldier>,
    >,
    mut links_query: Query<&mut CommsLinks, With<Soldier>>,
) {
    let snapshots: Vec<CommsSnapshot> = snapshots
        .iter()
        .map(|(entity, position, allegiance, voice)| CommsSnapshot {
            entity,
            position_m: position.0,
            side: allegiance.side,
            voice: voice.copied(),
        })
        .collect();

    for mut links in &mut links_query {
        links.links.clear();
    }

    for source in &snapshots {
        let Some(voice) = source.voice else {
            continue;
        };

        let Ok(mut links) = links_query.get_mut(source.entity) else {
            continue;
        };

        let range_sq = voice.range_m * voice.range_m;

        for target in &snapshots {
            if source.entity == target.entity || source.side != target.side {
                continue;
            }

            if source.position_m.distance_squared(target.position_m) <= range_sq {
                links.links.push(CommsLink {
                    target: target.entity,
                    kind: CommsKind::Voice,
                });
            }
        }
    }
}
