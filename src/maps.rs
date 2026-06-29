#![doc = include_str!("../docs/maps.md")]

use crate::gameplay::terrain::{HeightMap, TerrainDefinition};
use bevy::prelude::*;

pub struct MapDefinition {
    pub name: &'static str,
    pub size_m: Vec2,
    pub terrain: TerrainDefinition,
}

const DEMO_HEIGHTS_M: &[f32] = &[
    0.0, 0.0, 1.0, 2.0, 2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 5.0, 5.0, 3.0, 1.0, 0.0, 0.0, 1.0,
    3.0, 6.0, 9.0, 10.0, 7.0, 3.0, 1.0, 0.0, 1.0, 4.0, 8.0, 12.0, 14.0, 10.0, 5.0, 2.0, 1.0, 0.0,
    2.0, 5.0, 8.0, 10.0, 8.0, 4.0, 2.0, 0.0, 0.0, 1.0, 2.0, 4.0, 5.0, 4.0, 2.0, 1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0,
];

pub const DEMO_MAP: MapDefinition = MapDefinition {
    name: "Demo Map",
    size_m: Vec2::new(320.0, 240.0),
    terrain: TerrainDefinition::HeightMap(HeightMap {
        // Center this 9x7 sample field over the 320m x 240m map.
        origin_m: Vec2::new(-160.0, -120.0),
        sample_spacing_m: 40.0,
        width: 9,
        depth: 7,
        heights_m: DEMO_HEIGHTS_M,
    }),
};
