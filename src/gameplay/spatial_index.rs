//! Dense broad-phase spatial index for battlefield entities.
//!
//! The rendered map grid is presentation-facing. This grid is a separate,
//! coarser simulation index used to avoid testing every possible entity pair.

use crate::GameState;
use crate::actors::units::Soldier;
use crate::gameplay::map::BattlefieldMap;
use crate::gameplay::simulation::{SimulationSet, simulation_running};
use crate::gameplay::spatial::BattlefieldPosition;
use bevy::prelude::*;

/// Initial tuning value, intentionally independent of the rendered 10m grid.
pub const SPATIAL_CELL_SIZE_M: f32 = 50.0;

pub struct SpatialIndexPlugin;

impl Plugin for SpatialIndexPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BattlefieldSpatialGrid>().add_systems(
            FixedUpdate,
            rebuild_battlefield_spatial_grid
                .in_set(SimulationSet::SpatialIndex)
                .run_if(in_state(GameState::ScenarioScreen))
                .run_if(simulation_running),
        );
    }
}

/// Dense, map-bounded broad-phase index. Every soldier occupies exactly one
/// cell, so querying multiple cells never returns the same entity twice.
#[derive(Resource, Debug)]
pub struct BattlefieldSpatialGrid {
    origin_m: Vec2,
    map_size_m: Vec2,
    cell_size_m: f32,
    dimensions: UVec2,
    cells: Vec<Vec<Entity>>,
}

impl Default for BattlefieldSpatialGrid {
    fn default() -> Self {
        Self {
            origin_m: Vec2::ZERO,
            map_size_m: Vec2::ZERO,
            cell_size_m: SPATIAL_CELL_SIZE_M,
            dimensions: UVec2::ZERO,
            cells: Vec::new(),
        }
    }
}

impl BattlefieldSpatialGrid {
    fn configure(&mut self, map_size_m: Vec2) {
        debug_assert!(self.cell_size_m.is_finite() && self.cell_size_m > 0.0);
        debug_assert!(map_size_m.x.is_finite() && map_size_m.x > 0.0);
        debug_assert!(map_size_m.y.is_finite() && map_size_m.y > 0.0);

        let dimensions = UVec2::new(
            (map_size_m.x / self.cell_size_m).ceil().max(1.0) as u32,
            (map_size_m.y / self.cell_size_m).ceil().max(1.0) as u32,
        );

        if self.map_size_m == map_size_m && self.dimensions == dimensions {
            return;
        }

        self.origin_m = -map_size_m / 2.0;
        self.map_size_m = map_size_m;
        self.dimensions = dimensions;
        self.cells.clear();
        self.cells
            .resize_with((dimensions.x * dimensions.y) as usize, Vec::new);
    }

    fn clear(&mut self) {
        for cell in &mut self.cells {
            cell.clear();
        }
    }

    fn insert(&mut self, entity: Entity, position_m: Vec2) {
        let Some(index) = self.cell_index(position_m) else {
            return;
        };
        self.cells[index].push(entity);
    }

    fn cell_coordinates(&self, position_m: Vec2) -> Option<UVec2> {
        if self.dimensions == UVec2::ZERO || !position_m.is_finite() {
            return None;
        }

        let local = (position_m - self.origin_m) / self.cell_size_m;
        Some(UVec2::new(
            local.x.floor().clamp(0.0, (self.dimensions.x - 1) as f32) as u32,
            local.y.floor().clamp(0.0, (self.dimensions.y - 1) as f32) as u32,
        ))
    }

    fn cell_index(&self, position_m: Vec2) -> Option<usize> {
        let coordinates = self.cell_coordinates(position_m)?;
        Some((coordinates.y * self.dimensions.x + coordinates.x) as usize)
    }

    /// Visit every entity in a cell intersecting the axis-aligned bounds of a
    /// circular range. This is deliberately conservative: callers must still
    /// perform exact distance and other narrow-phase checks.
    pub fn visit_candidates(&self, center_m: Vec2, radius_m: f32, mut visitor: impl FnMut(Entity)) {
        if !center_m.is_finite() || !radius_m.is_finite() || radius_m < 0.0 {
            return;
        }

        let Some(min) = self.cell_coordinates(center_m - Vec2::splat(radius_m)) else {
            return;
        };
        let Some(max) = self.cell_coordinates(center_m + Vec2::splat(radius_m)) else {
            return;
        };

        for y in min.y..=max.y {
            for x in min.x..=max.x {
                let index = (y * self.dimensions.x + x) as usize;
                for &entity in &self.cells[index] {
                    visitor(entity);
                }
            }
        }
    }
}

pub(crate) fn rebuild_battlefield_spatial_grid(
    map: Res<BattlefieldMap>,
    soldiers: Query<(Entity, &BattlefieldPosition), With<Soldier>>,
    mut grid: ResMut<BattlefieldSpatialGrid>,
) {
    grid.configure(map.size_m);
    grid.clear();

    for (entity, position) in &soldiers {
        grid.insert(entity, position.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn configured_grid(map_size_m: Vec2) -> BattlefieldSpatialGrid {
        let mut grid = BattlefieldSpatialGrid::default();
        grid.configure(map_size_m);
        grid
    }

    #[test]
    fn dimensions_ceil_for_maps_not_divisible_by_cell_size() {
        let grid = configured_grid(Vec2::new(320.0, 240.0));
        assert_eq!(grid.dimensions, UVec2::new(7, 5));
        assert_eq!(grid.cells.len(), 35);
        assert_eq!(grid.origin_m, Vec2::new(-160.0, -120.0));
    }

    #[test]
    fn positions_map_to_expected_cells_and_edges_are_clamped() {
        let grid = configured_grid(Vec2::new(320.0, 240.0));
        assert_eq!(
            grid.cell_coordinates(Vec2::new(-160.0, -120.0)),
            Some(UVec2::ZERO)
        );
        assert_eq!(
            grid.cell_coordinates(Vec2::new(-110.0, -70.0)),
            Some(UVec2::new(1, 1))
        );
        assert_eq!(
            grid.cell_coordinates(Vec2::new(160.0, 120.0)),
            Some(UVec2::new(6, 4))
        );
        assert_eq!(
            grid.cell_coordinates(Vec2::new(-1_000.0, 1_000.0)),
            Some(UVec2::new(0, 4))
        );
    }

    #[test]
    fn range_query_is_conservative_and_does_not_duplicate_entities() {
        let mut world = World::new();
        let near = world.spawn_empty().id();
        let corner_false_positive = world.spawn_empty().id();
        let far = world.spawn_empty().id();
        let mut grid = configured_grid(Vec2::new(500.0, 500.0));

        grid.insert(near, Vec2::new(20.0, 0.0));
        // Inside the queried cell bounds but outside the exact 60m circle.
        grid.insert(corner_false_positive, Vec2::new(45.0, 45.0));
        grid.insert(far, Vec2::new(180.0, 180.0));

        let mut candidates = Vec::new();
        grid.visit_candidates(Vec2::ZERO, 60.0, |entity| candidates.push(entity));
        let unique: HashSet<_> = candidates.iter().copied().collect();

        assert!(unique.contains(&near));
        assert!(unique.contains(&corner_false_positive));
        assert!(!unique.contains(&far));
        assert_eq!(unique.len(), candidates.len());
    }

    #[test]
    fn rebuilding_cells_removes_an_entity_from_its_old_location() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let mut grid = configured_grid(Vec2::new(500.0, 500.0));

        grid.insert(entity, Vec2::new(-150.0, 0.0));
        grid.clear();
        grid.insert(entity, Vec2::new(150.0, 0.0));

        let mut old_candidates = Vec::new();
        grid.visit_candidates(Vec2::new(-150.0, 0.0), 20.0, |candidate| {
            old_candidates.push(candidate)
        });
        let mut new_candidates = Vec::new();
        grid.visit_candidates(Vec2::new(150.0, 0.0), 20.0, |candidate| {
            new_candidates.push(candidate)
        });

        assert!(!old_candidates.contains(&entity));
        assert!(new_candidates.contains(&entity));
    }

    #[test]
    fn every_entity_within_exact_range_is_returned() {
        let mut world = World::new();
        let mut grid = configured_grid(Vec2::new(500.0, 500.0));
        let positions = [
            Vec2::new(-249.0, -249.0),
            Vec2::new(-75.0, 20.0),
            Vec2::new(-1.0, -1.0),
            Vec2::ZERO,
            Vec2::new(49.9, 50.1),
            Vec2::new(130.0, -80.0),
            Vec2::new(249.0, 249.0),
        ];
        let entities: Vec<_> = positions
            .iter()
            .map(|&position| {
                let entity = world.spawn_empty().id();
                grid.insert(entity, position);
                entity
            })
            .collect();

        for center in [
            Vec2::ZERO,
            Vec2::new(-200.0, -200.0),
            Vec2::new(100.0, -50.0),
        ] {
            for radius in [0.0, 40.0, 150.0, 400.0] {
                let mut candidates = HashSet::new();
                grid.visit_candidates(center, radius, |entity| {
                    candidates.insert(entity);
                });

                for (&entity, &position) in entities.iter().zip(&positions) {
                    if center.distance_squared(position) <= radius * radius {
                        assert!(
                            candidates.contains(&entity),
                            "grid omitted {entity:?} at {position:?} from {center:?} radius {radius}"
                        );
                    }
                }
            }
        }
    }
}
