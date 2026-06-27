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
