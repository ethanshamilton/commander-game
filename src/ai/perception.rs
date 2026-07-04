#![doc = include_str!("../../docs/ai/perception.md")]

use crate::GameState;
use crate::actors::units::{Alive, Allegiance, Dead, Soldier};
use crate::gameplay::map::BattlefieldMap;
use crate::gameplay::simulation::{SimulationClock, SimulationSet};
use crate::gameplay::spatial::{BattlefieldPosition, Heading};
use crate::gameplay::terrain::TerrainHeight;
use crate::intel::ReportedLifeStatus;
use bevy::prelude::*;

const DEFAULT_VISUAL_RANGE_M: f32 = 150.0;
const DEFAULT_VISUAL_FOV_RADIANS: f32 = std::f32::consts::PI;
const DEFAULT_AUDITORY_RANGE_M: f32 = 40.0;
const DEFAULT_EYE_HEIGHT_M: f32 = 1.7;
const LOS_SAMPLE_SPACING_M: f32 = 2.0;
const LOS_TERRAIN_CLEARANCE_M: f32 = 0.1;
/// Per-observer expensive perception checks run at 4Hz on a 20Hz sim.
/// Cheap contact timestamp refreshes still run every tick so downstream systems
/// can keep using `last_seen_tick == clock.tick` as the "currently perceived" flag.
const SENSOR_SCAN_INTERVAL_TICKS: u64 = 5;

pub struct PerceptionPlugin;

impl Plugin for PerceptionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                stamp_sensor_changes,
                update_visual_perception,
                update_auditory_perception,
            )
                .chain()
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
pub struct AuditorySensor {
    pub range_m: f32,
}

impl Default for AuditorySensor {
    fn default() -> Self {
        Self {
            range_m: DEFAULT_AUDITORY_RANGE_M,
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
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct SensorStamp {
    /// Last simulation tick where this entity changed in a way perception cares about.
    pub tick: u64,
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct SensorScanState {
    last_visual_scan_tick: Option<u64>,
    last_visual_processed_tick: Option<u64>,
    last_auditory_scan_tick: Option<u64>,
    last_auditory_processed_tick: Option<u64>,
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

impl PerceptionMemory {
    pub fn unique_contact_count(&self) -> usize {
        self.contacts
            .iter()
            .map(|contact| contact.target)
            .collect::<std::collections::HashSet<_>>()
            .len()
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Contact {
    pub target: Entity,
    pub last_seen_position_m: Vec2,
    pub last_seen_time_s: f32,
    pub last_seen_tick: u64,
    pub confidence: f32,
    pub observed_life_status: ReportedLifeStatus,
    pub kind: ContactKind,
    pub contact_type: ContactType,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactKind {
    Visual,
    Auditory,
    Radar,
    Unknown,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactType {
    Friendly,
    Hostile,
    Neutral,
    Unknown,
}

fn stamp_sensor_changes(
    clock: Res<SimulationClock>,
    mut changed_units: Query<
        &mut SensorStamp,
        (
            With<Soldier>,
            Or<(
                Changed<BattlefieldPosition>,
                Changed<Heading>,
                Changed<SensorSignature>,
                Added<Dead>,
            )>,
        ),
    >,
) {
    for mut stamp in &mut changed_units {
        stamp.tick = clock.tick;
    }
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
            &SensorStamp,
            &mut SensorScanState,
            &mut PerceptionMemory,
        ),
        (With<Soldier>, With<Alive>),
    >,
    targets: Query<
        (
            Entity,
            &BattlefieldPosition,
            Option<&EyeHeight>,
            &SensorSignature,
            &Allegiance,
            &SensorStamp,
            Option<&Dead>,
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
        observer_stamp,
        mut scan_state,
        mut memory,
    ) in &mut observers
    {
        let previous_processed_tick = scan_state.last_visual_processed_tick;
        let last_scan_tick = scan_state.last_visual_scan_tick;
        let scan_due = last_scan_tick.is_none() || should_scan_sensor(clock.tick, observer);
        let mut rechecked_targets = Vec::new();

        if scan_due {
            let observer_changed_since_scan = last_scan_tick
                .is_none_or(|last_scan_tick| observer_stamp.tick > last_scan_tick);
            let observer_position_m = observer_position.0;

            for (
                target,
                target_position,
                target_eye_height,
                signature,
                target_allegiance,
                target_stamp,
                target_dead,
            ) in &targets
            {
                if target == observer {
                    continue;
                }

                let target_changed_since_scan = last_scan_tick
                    .is_none_or(|last_scan_tick| target_stamp.tick > last_scan_tick);
                if !observer_changed_since_scan && !target_changed_since_scan {
                    continue;
                }

                rechecked_targets.push(target);

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
                        observed_life_status: if target_dead.is_some() {
                            ReportedLifeStatus::Dead
                        } else {
                            ReportedLifeStatus::Alive
                        },
                        kind: ContactKind::Visual,
                        contact_type: if target_allegiance.side == observer_allegiance.side {
                            ContactType::Friendly
                        } else {
                            ContactType::Hostile
                        },
                    },
                );
            }

            scan_state.last_visual_scan_tick = Some(clock.tick);
        }

        if let Some(previous_processed_tick) = previous_processed_tick {
            refresh_current_contacts(
                &mut memory,
                ContactKind::Visual,
                previous_processed_tick,
                clock.tick,
                clock.elapsed_s,
                &rechecked_targets,
            );
        }
        scan_state.last_visual_processed_tick = Some(clock.tick);
    }
}

pub fn update_auditory_perception(
    clock: Res<SimulationClock>,
    mut observers: Query<
        (
            Entity,
            &BattlefieldPosition,
            &AuditorySensor,
            &Allegiance,
            &SensorStamp,
            &mut SensorScanState,
            &mut PerceptionMemory,
        ),
        (With<Soldier>, With<Alive>),
    >,
    targets: Query<
        (
            Entity,
            &BattlefieldPosition,
            &SensorSignature,
            &Allegiance,
            &SensorStamp,
            Option<&Dead>,
        ),
        With<Soldier>,
    >,
) {
    for (
        observer,
        observer_position,
        auditory_sensor,
        observer_allegiance,
        observer_stamp,
        mut scan_state,
        mut memory,
    ) in &mut observers
    {
        let previous_processed_tick = scan_state.last_auditory_processed_tick;
        let last_scan_tick = scan_state.last_auditory_scan_tick;
        let scan_due = last_scan_tick.is_none() || should_scan_sensor(clock.tick, observer);
        let mut rechecked_targets = Vec::new();

        if scan_due {
            let observer_changed_since_scan = last_scan_tick
                .is_none_or(|last_scan_tick| observer_stamp.tick > last_scan_tick);
            let observer_position_m = observer_position.0;

            for (target, target_position, signature, target_allegiance, target_stamp, target_dead) in
                &targets
            {
                if target == observer {
                    continue;
                }

                let target_changed_since_scan = last_scan_tick
                    .is_none_or(|last_scan_tick| target_stamp.tick > last_scan_tick);
                if !observer_changed_since_scan && !target_changed_since_scan {
                    continue;
                }

                rechecked_targets.push(target);

                if target_dead.is_some() || signature.acoustic <= 0.0 {
                    continue;
                }

                let effective_range_m = auditory_sensor.range_m * signature.acoustic;
                let target_position_m = target_position.0;

                if observer_position_m.distance_squared(target_position_m)
                    > effective_range_m * effective_range_m
                {
                    continue;
                }

                upsert_contact(
                    &mut memory,
                    Contact {
                        target,
                        last_seen_position_m: target_position_m,
                        last_seen_time_s: clock.elapsed_s,
                        last_seen_tick: clock.tick,
                        confidence: signature.acoustic.clamp(0.0, 1.0),
                        observed_life_status: ReportedLifeStatus::Alive,
                        kind: ContactKind::Auditory,
                        contact_type: if target_allegiance.side == observer_allegiance.side {
                            ContactType::Friendly
                        } else {
                            ContactType::Hostile
                        },
                    },
                );
            }

            scan_state.last_auditory_scan_tick = Some(clock.tick);
        }

        if let Some(previous_processed_tick) = previous_processed_tick {
            refresh_current_contacts(
                &mut memory,
                ContactKind::Auditory,
                previous_processed_tick,
                clock.tick,
                clock.elapsed_s,
                &rechecked_targets,
            );
        }
        scan_state.last_auditory_processed_tick = Some(clock.tick);
    }
}

fn should_scan_sensor(tick: u64, observer: Entity) -> bool {
    (tick + observer.index().index() as u64) % SENSOR_SCAN_INTERVAL_TICKS == 0
}

fn refresh_current_contacts(
    memory: &mut PerceptionMemory,
    kind: ContactKind,
    previous_processed_tick: u64,
    current_tick: u64,
    current_time_s: f32,
    rechecked_targets: &[Entity],
) {
    for contact in &mut memory.contacts {
        if contact.kind != kind
            || contact.last_seen_tick != previous_processed_tick
            || rechecked_targets.contains(&contact.target)
        {
            continue;
        }

        contact.last_seen_tick = current_tick;
        contact.last_seen_time_s = current_time_s;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::terrain::TerrainDefinition;

    /// Flat ground at 0 with a cylindrical wall: analytic LOS oracle.
    struct Wall {
        center_m: Vec2,
        radius_m: f32,
        height_m: f32,
    }

    impl TerrainHeight for Wall {
        fn height_at_m(&self, position_m: Vec2) -> f32 {
            if position_m.distance(self.center_m) <= self.radius_m {
                self.height_m
            } else {
                0.0
            }
        }
    }

    const EYE_M: f32 = 1.7;

    #[test]
    fn los_on_flat_terrain_is_always_clear() {
        let flat = TerrainDefinition::Flat { height_m: 0.0 };
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(120.0, -45.0);
        assert!(has_line_of_sight(&flat, a, EYE_M, b, EYE_M));
        // zero distance is defined as visible
        assert!(has_line_of_sight(&flat, a, EYE_M, a, EYE_M));
    }

    #[test]
    fn tall_wall_between_observers_blocks_los() {
        let wall = Wall {
            center_m: Vec2::new(20.0, 0.0),
            radius_m: 2.0,
            height_m: 10.0,
        };
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(40.0, 0.0);
        assert!(!has_line_of_sight(&wall, a, EYE_M, b, EYE_M));
    }

    #[test]
    fn elevated_observers_see_over_wall() {
        let wall = Wall {
            center_m: Vec2::new(20.0, 0.0),
            radius_m: 2.0,
            height_m: 10.0,
        };
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(40.0, 0.0);
        // eyes at 12m (e.g. towers): sightline stays above the 10m wall
        assert!(has_line_of_sight(&wall, a, 12.0, b, 12.0));
    }

    #[test]
    fn los_is_symmetric_for_equal_eye_heights() {
        // asymmetric wall placement so a directional sampling bug would show
        let wall = Wall {
            center_m: Vec2::new(5.0, 0.0),
            radius_m: 2.0,
            height_m: 10.0,
        };
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(40.0, 0.0);
        assert_eq!(
            has_line_of_sight(&wall, a, EYE_M, b, EYE_M),
            has_line_of_sight(&wall, b, EYE_M, a, EYE_M),
        );
    }

    /// Documents (rather than endorses) current behavior: below the 2m sample
    /// spacing, no intermediate samples are taken, so even a razor-thin wall
    /// between two adjacent units is ignored. If this test starts failing you
    /// have *fixed* the edge case and should update the LOS docs.
    #[test]
    fn los_below_sample_spacing_never_blocks() {
        let wall = Wall {
            center_m: Vec2::new(0.75, 0.0),
            radius_m: 0.5,
            height_m: 100.0,
        };
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(1.5, 0.0);
        assert!(has_line_of_sight(&wall, a, EYE_M, b, EYE_M));
    }

    #[test]
    fn upsert_contact_updates_in_place_per_sensor_kind() {
        let mut memory = PerceptionMemory::default();
        let mut world = bevy::ecs::world::World::new();
        let target = world.spawn_empty().id();

        let contact = |kind, tick| Contact {
            target,
            last_seen_position_m: Vec2::ZERO,
            last_seen_time_s: 0.0,
            last_seen_tick: tick,
            confidence: 1.0,
            observed_life_status: ReportedLifeStatus::Alive,
            kind,
            contact_type: ContactType::Hostile,
        };

        upsert_contact(&mut memory, contact(ContactKind::Visual, 1));
        upsert_contact(&mut memory, contact(ContactKind::Auditory, 1));
        upsert_contact(&mut memory, contact(ContactKind::Visual, 2));

        // same target + same kind replaces; different kind coexists
        assert_eq!(memory.contacts.len(), 2);
        assert_eq!(memory.unique_contact_count(), 1);
        let visual = memory
            .contacts
            .iter()
            .find(|c| c.kind == ContactKind::Visual)
            .unwrap();
        assert_eq!(visual.last_seen_tick, 2);
    }
}
