use crate::gameplay::measurements::meters;
use crate::gameplay::terrain::TerrainDefinition;
use crate::maps::MapDefinition;
use bevy::prelude::*;

pub const GRID_SPACING_METERS: f32 = 10.0;

#[derive(Resource, Debug, Clone)]
pub struct BattlefieldMap {
    pub size_m: Vec2,
    pub cells: UVec2,
    pub cell_size: f32,
    pub terrain: TerrainDefinition,
}

impl BattlefieldMap {
    pub fn from_definition(map: &MapDefinition) -> Self {
        Self {
            size_m: map.size_m,
            cells: (map.size_m / GRID_SPACING_METERS).as_uvec2(),
            cell_size: meters(GRID_SPACING_METERS),
            terrain: map.terrain,
        }
    }
}

impl Default for BattlefieldMap {
    fn default() -> Self {
        Self {
            size_m: Vec2::new(320.0, 240.0),
            cells: UVec2::new(32, 24),
            cell_size: meters(GRID_SPACING_METERS),
            terrain: TerrainDefinition::Flat { height_m: 0.0 },
        }
    }
}
