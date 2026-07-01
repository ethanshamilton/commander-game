#![doc = include_str!("../../docs/gameplay/terrain.md")]

use bevy::prelude::*;

pub trait TerrainHeight {
    fn height_at_m(&self, position_m: Vec2) -> f32;
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum TerrainDefinition {
    Flat { height_m: f32 },
    HeightMap(HeightMap),
}

#[derive(Debug, Clone, Copy)]
pub struct HeightMap {
    pub origin_m: Vec2,
    pub sample_spacing_m: f32,
    pub width: usize,
    pub depth: usize,
    pub heights_m: &'static [f32],
}

impl HeightMap {
    fn height_at_sample(&self, x: usize, z: usize) -> f32 {
        debug_assert!(x < self.width);
        debug_assert!(z < self.depth);
        debug_assert_eq!(self.heights_m.len(), self.width * self.depth);

        self.heights_m[z * self.width + x]
    }
}

impl TerrainHeight for TerrainDefinition {
    fn height_at_m(&self, position_m: Vec2) -> f32 {
        match self {
            TerrainDefinition::Flat { height_m } => *height_m,
            TerrainDefinition::HeightMap(heightmap) => heightmap.height_at_m(position_m),
        }
    }
}

impl TerrainHeight for HeightMap {
    fn height_at_m(&self, position_m: Vec2) -> f32 {
        debug_assert!(self.width >= 2);
        debug_assert!(self.depth >= 2);
        debug_assert!(self.sample_spacing_m > 0.0);

        let local = (position_m - self.origin_m) / self.sample_spacing_m;
        let x = local.x.clamp(0.0, (self.width - 1) as f32);
        let z = local.y.clamp(0.0, (self.depth - 1) as f32);

        let x0 = x.floor() as usize;
        let z0 = z.floor() as usize;
        let x1 = (x0 + 1).min(self.width - 1);
        let z1 = (z0 + 1).min(self.depth - 1);

        let tx = x - x0 as f32;
        let tz = z - z0 as f32;

        let h00 = self.height_at_sample(x0, z0);
        let h10 = self.height_at_sample(x1, z0);
        let h01 = self.height_at_sample(x0, z1);
        let h11 = self.height_at_sample(x1, z1);

        let h0 = h00.lerp(h10, tx);
        let h1 = h01.lerp(h11, tx);

        h0.lerp(h1, tz)
    }
}

pub const LEVEL_HEIGHT_M: f32 = 5.0;

#[allow(dead_code)]
pub fn elevation_level_at(terrain: &impl TerrainHeight, position_m: Vec2) -> i32 {
    (terrain.height_at_m(position_m) / LEVEL_HEIGHT_M).floor() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deliberately non-square (3 wide, 2 deep): an x/z indexing swap is
    /// invisible on square grids.
    fn test_heightmap() -> HeightMap {
        HeightMap {
            origin_m: Vec2::ZERO,
            sample_spacing_m: 10.0,
            width: 3,
            depth: 2,
            // row z=0: 0, 10, 20 | row z=1: 30, 40, 50
            heights_m: &[0.0, 10.0, 20.0, 30.0, 40.0, 50.0],
        }
    }

    #[test]
    fn heightmap_is_exact_at_sample_points() {
        let hm = test_heightmap();
        assert_eq!(hm.height_at_m(Vec2::new(0.0, 0.0)), 0.0);
        assert_eq!(hm.height_at_m(Vec2::new(20.0, 0.0)), 20.0);
        assert_eq!(hm.height_at_m(Vec2::new(0.0, 10.0)), 30.0);
        // corner of the non-square grid: catches z * width + x vs x * depth + z
        assert_eq!(hm.height_at_m(Vec2::new(20.0, 10.0)), 50.0);
    }

    #[test]
    fn heightmap_bilinear_midpoint_matches_hand_computation() {
        let hm = test_heightmap();
        // cell (0..10, 0..10) has corners 0, 10, 30, 40 -> mean at center = 20
        assert_eq!(hm.height_at_m(Vec2::new(5.0, 5.0)), 20.0);
        // interpolation along x only, z=0 row: halfway between 10 and 20
        assert_eq!(hm.height_at_m(Vec2::new(15.0, 0.0)), 15.0);
    }

    #[test]
    fn heightmap_clamps_out_of_bounds_queries_to_border() {
        let hm = test_heightmap();
        assert_eq!(hm.height_at_m(Vec2::new(-100.0, -100.0)), 0.0);
        assert_eq!(hm.height_at_m(Vec2::new(1000.0, 1000.0)), 50.0);
        // out of bounds on one axis only
        assert_eq!(hm.height_at_m(Vec2::new(1000.0, 0.0)), 20.0);
    }

    #[test]
    fn flat_terrain_is_uniform_and_levels_floor_correctly() {
        let flat = TerrainDefinition::Flat { height_m: 7.0 };
        assert_eq!(flat.height_at_m(Vec2::new(-500.0, 12345.0)), 7.0);
        // LEVEL_HEIGHT_M = 5.0: height 7 -> level 1
        assert_eq!(elevation_level_at(&flat, Vec2::ZERO), 1);
        // negative heights floor downward, not toward zero
        let below = TerrainDefinition::Flat { height_m: -0.1 };
        assert_eq!(elevation_level_at(&below, Vec2::ZERO), -1);
    }
}
