use crate::GameState;
use crate::gameplay::components::{BattlefieldPosition, Heading};
use crate::gameplay::map::BattlefieldMap;
use crate::gameplay::simulation::{SimulationClock, SimulationSet};
use crate::gameplay::terrain::TerrainHeight;
use crate::units::{Allegiance, Soldier};
use bevy::prelude::*;

const DEFAULT_VISUAL_RANGE_M: f32 = 150.0;
const DEFAULT_VISUAL_FOV_RADIANS: f32 = std::f32::consts::PI;
const DEFAULT_EYE_HEIGHT_M: f32 = 1.7;
const LOS_SAMPLE_SPACING_M: f32 = 2.0;
const LOS_TERRAIN_CLEARANCE_M: f32 = 0.1;

pub struct PerceptionPlugin;

impl Plugin for PerceptionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            update_visual_perception
                .in_set(SimulationSet::Sensors)
                .run_if(in_state(GameState::MissionScreen)),
        );
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct VisualSensor {
    pub range_m: f32,
    pub fov_radians: f32,
}

impl Default for VisualSensor {
    fn default() -> Self {
        Self {
            range_m: DEFAULT_VISUAL_RANGE_M,
            fov_radians: DEFAULT_VISUAL_FOV_RADIANS,
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct EyeHeight {
    pub height_m: f32,
}

impl Default for EyeHeight {
    fn default() -> Self {
        Self {
            height_m: DEFAULT_EYE_HEIGHT_M,
        }
    }
}

#[allow(dead_code)]
#[derive(Component, Debug, Clone, Copy)]
pub struct SensorSignature {
    pub visual: f32,
    pub infrared: f32,
    pub radar_cross_section: f32,
    pub acoustic: f32,
}

impl Default for SensorSignature {
    fn default() -> Self {
        Self {
            visual: 1.0,
            infrared: 1.0,
            radar_cross_section: 1.0,
            acoustic: 1.0,
        }
    }
}

#[derive(Component, Debug, Default)]
pub struct PerceptionMemory {
    pub contacts: Vec<Contact>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Contact {
    pub target: Entity,
    pub last_seen_position_m: Vec2,
    pub last_seen_time_s: f32,
    pub last_seen_tick: u64,
    pub confidence: f32,
    pub kind: ContactKind,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactKind {
    Visual,
    Audio,
    Radar,
    Unknown,
}

pub fn update_visual_perception(
    clock: Res<SimulationClock>,
    map: Res<BattlefieldMap>,
    mut observers: Query<
        (
            Entity,
            &BattlefieldPosition,
            &Heading,
            &VisualSensor,
            &EyeHeight,
            &Allegiance,
            &mut PerceptionMemory,
        ),
        With<Soldier>,
    >,
    targets: Query<
        (
            Entity,
            &BattlefieldPosition,
            Option<&EyeHeight>,
            &SensorSignature,
            &Allegiance,
        ),
        With<Soldier>,
    >,
) {
    for (
        observer,
        observer_position,
        Heading(observer_heading),
        visual_sensor,
        observer_eye_height,
        observer_allegiance,
        mut memory,
    ) in &mut observers
    {
        let observer_position_m = observer_position.0;

        for (target, target_position, target_eye_height, signature, target_allegiance) in &targets {
            if target == observer || target_allegiance.side == observer_allegiance.side {
                continue;
            }

            if signature.visual <= 0.0 {
                continue;
            }

            let target_position_m = target_position.0;
            if !is_in_visual_cone(
                observer_position_m,
                *observer_heading,
                target_position_m,
                visual_sensor,
            ) {
                continue;
            }

            let target_eye_height_m = target_eye_height
                .map(|eye_height| eye_height.height_m)
                .unwrap_or(DEFAULT_EYE_HEIGHT_M);

            if !has_line_of_sight(
                &map.terrain,
                observer_position_m,
                observer_eye_height.height_m,
                target_position_m,
                target_eye_height_m,
            ) {
                continue;
            }

            upsert_contact(
                &mut memory,
                Contact {
                    target,
                    last_seen_position_m: target_position_m,
                    last_seen_time_s: clock.elapsed_s,
                    last_seen_tick: clock.tick,
                    confidence: signature.visual.clamp(0.0, 1.0),
                    kind: ContactKind::Visual,
                },
            );
        }
    }
}

fn is_in_visual_cone(
    observer_position_m: Vec2,
    observer_heading: f32,
    target_position_m: Vec2,
    sensor: &VisualSensor,
) -> bool {
    let offset = target_position_m - observer_position_m;
    let distance_m = offset.length();

    if distance_m > sensor.range_m || distance_m == 0.0 {
        return false;
    }

    let facing = Vec2::from_angle(observer_heading);
    let target_direction = offset / distance_m;
    let min_dot = (sensor.fov_radians / 2.0).cos();

    facing.dot(target_direction) >= min_dot
}

pub fn has_line_of_sight(
    terrain: &impl TerrainHeight,
    observer_position_m: Vec2,
    observer_eye_height_m: f32,
    target_position_m: Vec2,
    target_eye_height_m: f32,
) -> bool {
    let offset = target_position_m - observer_position_m;
    let distance_m = offset.length();

    if distance_m == 0.0 {
        return true;
    }

    let observer_height_m = terrain.height_at_m(observer_position_m) + observer_eye_height_m;
    let target_height_m = terrain.height_at_m(target_position_m) + target_eye_height_m;
    let sample_count = (distance_m / LOS_SAMPLE_SPACING_M).floor() as usize;

    for sample in 1..sample_count {
        let t = sample as f32 / sample_count as f32;
        let position_m = observer_position_m.lerp(target_position_m, t);
        let sightline_height_m = observer_height_m.lerp(target_height_m, t);
        let terrain_height_m = terrain.height_at_m(position_m);

        if terrain_height_m > sightline_height_m - LOS_TERRAIN_CLEARANCE_M {
            return false;
        }
    }

    true
}

fn upsert_contact(memory: &mut PerceptionMemory, contact: Contact) {
    if let Some(existing) = memory
        .contacts
        .iter_mut()
        .find(|existing| existing.target == contact.target && existing.kind == contact.kind)
    {
        *existing = contact;
    } else {
        memory.contacts.push(contact);
    }
}
