#![doc = include_str!("../../docs/ai/perception.md")]

use crate::GameState;
use crate::actors::units::{Alive, Allegiance, Dead, Soldier};
use crate::gameplay::map::BattlefieldMap;
use crate::gameplay::simulation::{SimulationClock, SimulationSet};
use crate::gameplay::spatial::{BattlefieldPosition, Heading};
use crate::gameplay::spatial_index::BattlefieldSpatialGrid;
use crate::gameplay::terrain::TerrainHeight;
use crate::intel::ReportedLifeStatus;
use bevy::prelude::*;

const DEFAULT_VISUAL_RANGE_M: f32 = 150.0;
const DEFAULT_VISUAL_FOV_RADIANS: f32 = std::f32::consts::PI;
const DEFAULT_AUDITORY_RANGE_M: f32 = 40.0;
const DEFAULT_EYE_HEIGHT_M: f32 = 1.7;
const LOS_SAMPLE_SPACING_M: f32 = 2.0;
const LOS_TERRAIN_CLEARANCE_M: f32 = 0.1;

pub struct PerceptionPlugin;

impl Plugin for PerceptionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (update_visual_perception, update_auditory_perception)
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

pub fn update_visual_perception(
    clock: Res<SimulationClock>,
    map: Res<BattlefieldMap>,
    grid: Res<BattlefieldSpatialGrid>,
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
        (With<Soldier>, With<Alive>),
    >,
    targets: Query<
        (
            &BattlefieldPosition,
            Option<&EyeHeight>,
            &SensorSignature,
            &Allegiance,
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
        mut memory,
    ) in &mut observers
    {
        let observer_position_m = observer_position.0;

        grid.visit_candidates(observer_position_m, visual_sensor.range_m, |target| {
            if target == observer {
                return;
            }

            let Ok((
                target_position,
                target_eye_height,
                signature,
                target_allegiance,
                target_dead,
            )) = targets.get(target)
            else {
                return;
            };

            if signature.visual <= 0.0 {
                return;
            }

            let target_position_m = target_position.0;
            if !is_in_visual_cone(
                observer_position_m,
                *observer_heading,
                target_position_m,
                visual_sensor,
            ) {
                return;
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
                return;
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
        });
    }
}

pub fn update_auditory_perception(
    clock: Res<SimulationClock>,
    grid: Res<BattlefieldSpatialGrid>,
    mut observers: Query<
        (
            Entity,
            &BattlefieldPosition,
            &AuditorySensor,
            &Allegiance,
            &mut PerceptionMemory,
        ),
        (With<Soldier>, With<Alive>),
    >,
    targets: Query<
        (
            &BattlefieldPosition,
            &SensorSignature,
            &Allegiance,
            Option<&Dead>,
        ),
        With<Soldier>,
    >,
) {
    // Acoustic signatures scale effective range per target. Find the largest
    // current multiplier once so the broad phase remains conservative without
    // imposing a new bound on signature values.
    let max_acoustic_signature = targets
        .iter()
        .filter_map(|(_, signature, _, dead)| {
            (dead.is_none() && signature.acoustic.is_finite() && signature.acoustic > 0.0)
                .then_some(signature.acoustic)
        })
        .fold(0.0_f32, f32::max);

    for (observer, observer_position, auditory_sensor, observer_allegiance, mut memory) in
        &mut observers
    {
        let observer_position_m = observer_position.0;
        let broad_phase_range_m = auditory_sensor.range_m * max_acoustic_signature;

        grid.visit_candidates(observer_position_m, broad_phase_range_m, |target| {
            if target == observer {
                return;
            }

            let Ok((target_position, signature, target_allegiance, target_dead)) =
                targets.get(target)
            else {
                return;
            };

            if target_dead.is_some() || signature.acoustic <= 0.0 {
                return;
            }

            let effective_range_m = auditory_sensor.range_m * signature.acoustic;
            let target_position_m = target_position.0;

            if observer_position_m.distance_squared(target_position_m)
                > effective_range_m * effective_range_m
            {
                return;
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
        });
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
    use crate::actors::units::{Rank, Role, Side};
    use crate::gameplay::spatial_index::{
        BattlefieldSpatialGrid, rebuild_battlefield_spatial_grid,
    };
    use crate::gameplay::terrain::TerrainDefinition;
    use bevy::ecs::system::RunSystemOnce;

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

    #[test]
    fn spatial_grid_drives_visual_and_auditory_detection() {
        let mut world = World::new();
        world.insert_resource(SimulationClock {
            tick: 7,
            elapsed_s: 0.35,
            ..default()
        });
        world.insert_resource(BattlefieldMap::default());
        world.insert_resource(BattlefieldSpatialGrid::default());

        let observer = world
            .spawn((
                Soldier {
                    rank: Rank::Private,
                    role: Role::Rifleman,
                },
                Alive,
                Allegiance { side: Side::Blue },
                BattlefieldPosition(Vec2::ZERO),
                Heading(0.0),
                VisualSensor::default(),
                AuditorySensor::default(),
                EyeHeight::default(),
                SensorSignature::default(),
                PerceptionMemory::default(),
            ))
            .id();
        let target = world
            .spawn((
                Soldier {
                    rank: Rank::Private,
                    role: Role::Rifleman,
                },
                Alive,
                Allegiance { side: Side::Red },
                BattlefieldPosition(Vec2::new(30.0, 0.0)),
                EyeHeight::default(),
                SensorSignature::default(),
                PerceptionMemory::default(),
            ))
            .id();

        world
            .run_system_once(rebuild_battlefield_spatial_grid)
            .unwrap();
        world.run_system_once(update_visual_perception).unwrap();
        world.run_system_once(update_auditory_perception).unwrap();

        let memory = world.get::<PerceptionMemory>(observer).unwrap();
        assert!(memory.contacts.iter().any(|contact| {
            contact.target == target
                && contact.kind == ContactKind::Visual
                && contact.contact_type == ContactType::Hostile
        }));
        assert!(memory.contacts.iter().any(|contact| {
            contact.target == target
                && contact.kind == ContactKind::Auditory
                && contact.contact_type == ContactType::Hostile
        }));
    }
}
