#![doc = include_str!("../../../docs/gameplay/rendering/map.md")]

use super::RenderingSet;
use crate::GameState;
use crate::gameplay::map::BattlefieldMap;
use crate::gameplay::measurements::meters;
use crate::gameplay::terrain::TerrainHeight;
use bevy::prelude::*;

pub struct MapRenderingPlugin;

impl Plugin for MapRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (draw_battlefield_grid, draw_topography)
                .chain()
                .in_set(RenderingSet::Map)
                .run_if(in_state(GameState::ScenarioScreen)),
        );
    }
}

fn draw_battlefield_grid(mut gizmos: Gizmos, map: Res<BattlefieldMap>) {
    let grid_color = Color::srgb(0.16, 0.16, 0.16);
    let border_color = Color::srgb(0.28, 0.28, 0.28);
    let axis_color = Color::srgb(0.22, 0.22, 0.22);

    gizmos
        .grid_2d(
            Isometry2d::IDENTITY,
            map.cells,
            Vec2::splat(map.cell_size),
            grid_color,
        )
        .outer_edges();

    let size = map.size_m.map(meters);
    gizmos.rect_2d(Isometry2d::IDENTITY, size, border_color);

    gizmos.line_2d(
        Vec2::new(-size.x / 2.0, 0.0),
        Vec2::new(size.x / 2.0, 0.0),
        axis_color,
    );
    gizmos.line_2d(
        Vec2::new(0.0, -size.y / 2.0),
        Vec2::new(0.0, size.y / 2.0),
        axis_color,
    );
}

const TOPOGRAPHY_SAMPLE_SPACING_METERS: f32 = 2.5;
const CONTOUR_INTERVAL_METERS: f32 = 2.0;
const MAX_CONTOUR_HEIGHT_METERS: f32 = 20.0;

fn draw_topography(mut gizmos: Gizmos, map: Res<BattlefieldMap>) {
    let contour_color = Color::srgb(0.72, 0.72, 0.72);
    let min_m = -map.size_m / 2.0;
    let cells = (map.size_m / TOPOGRAPHY_SAMPLE_SPACING_METERS).as_uvec2();

    for z in 0..cells.y {
        for x in 0..cells.x {
            let p00 = min_m
                + Vec2::new(
                    x as f32 * TOPOGRAPHY_SAMPLE_SPACING_METERS,
                    z as f32 * TOPOGRAPHY_SAMPLE_SPACING_METERS,
                );
            let p10 = p00 + Vec2::X * TOPOGRAPHY_SAMPLE_SPACING_METERS;
            let p01 = p00 + Vec2::Y * TOPOGRAPHY_SAMPLE_SPACING_METERS;
            let p11 = p00 + Vec2::splat(TOPOGRAPHY_SAMPLE_SPACING_METERS);

            let h00 = map.terrain.height_at_m(p00);
            let h10 = map.terrain.height_at_m(p10);
            let h01 = map.terrain.height_at_m(p01);
            let h11 = map.terrain.height_at_m(p11);

            let mut contour = CONTOUR_INTERVAL_METERS;
            while contour <= MAX_CONTOUR_HEIGHT_METERS {
                draw_contour_cell(
                    &mut gizmos,
                    contour_color,
                    contour,
                    [(p00, h00), (p10, h10), (p11, h11), (p01, h01)],
                );
                contour += CONTOUR_INTERVAL_METERS;
            }
        }
    }
}

fn draw_contour_cell(gizmos: &mut Gizmos, color: Color, contour_m: f32, corners: [(Vec2, f32); 4]) {
    let mut intersections = [Vec2::ZERO; 4];
    let mut count = 0;

    push_contour_intersection(
        corners[0],
        corners[1],
        contour_m,
        &mut intersections,
        &mut count,
    );
    push_contour_intersection(
        corners[1],
        corners[2],
        contour_m,
        &mut intersections,
        &mut count,
    );
    push_contour_intersection(
        corners[2],
        corners[3],
        contour_m,
        &mut intersections,
        &mut count,
    );
    push_contour_intersection(
        corners[3],
        corners[0],
        contour_m,
        &mut intersections,
        &mut count,
    );

    for segment in intersections[..count].chunks_exact(2) {
        gizmos.line_2d(segment[0].map(meters), segment[1].map(meters), color);
    }
}

fn push_contour_intersection(
    a: (Vec2, f32),
    b: (Vec2, f32),
    contour_m: f32,
    intersections: &mut [Vec2; 4],
    count: &mut usize,
) {
    let (pa, ha) = a;
    let (pb, hb) = b;

    if (ha < contour_m && hb >= contour_m) || (hb < contour_m && ha >= contour_m) {
        let t = (contour_m - ha) / (hb - ha);
        intersections[*count] = pa.lerp(pb, t);
        *count += 1;
    }
}
