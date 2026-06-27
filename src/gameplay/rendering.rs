use crate::gameplay::components::{BattlefieldPosition, Heading};
use crate::units::{Allegiance, Side, Soldier};
use crate::GameState;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;

pub struct GameplayRenderingPlugin;

impl Plugin for GameplayRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BattlefieldMap>().add_systems(
            Update,
            (zoom_battlefield_camera, draw_battlefield_grid, draw_units)
                .run_if(in_state(GameState::MissionScreen)),
        );
    }
}

pub const BEVY_UNITS_PER_METER: f32 = 10.0;
pub const GRID_SPACING_METERS: f32 = 10.0;

pub const fn meters(value: f32) -> f32 {
    value * BEVY_UNITS_PER_METER
}

#[allow(dead_code)]
pub const fn to_meters(bevy_units: f32) -> f32 {
    bevy_units / BEVY_UNITS_PER_METER
}

#[derive(Resource, Debug, Clone)]
pub struct BattlefieldMap {
    pub cells: UVec2,
    pub cell_size: f32,
}

impl Default for BattlefieldMap {
    fn default() -> Self {
        Self {
            cells: UVec2::new(32, 24),
            cell_size: meters(GRID_SPACING_METERS),
        }
    }
}

const MIN_CAMERA_SCALE: f32 = 0.25;
const MAX_CAMERA_SCALE: f32 = 4.0;
const ZOOM_SENSITIVITY: f32 = 0.2;

fn zoom_battlefield_camera(
    scroll: Res<AccumulatedMouseScroll>,
    mut cameras: Query<&mut Projection, With<Camera2d>>,
) {
    if scroll.delta.y == 0.0 {
        return;
    }

    let scroll_lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR,
    };

    // Orthographic scale is inverse zoom: smaller scale means closer in.
    let zoom_factor = (-scroll_lines * ZOOM_SENSITIVITY).exp();

    for mut projection in &mut cameras {
        if let Projection::Orthographic(orthographic) = projection.as_mut() {
            orthographic.scale = (orthographic.scale * zoom_factor)
                .clamp(MIN_CAMERA_SCALE, MAX_CAMERA_SCALE);
        }
    }
}

fn draw_battlefield_grid(mut gizmos: Gizmos, map: Res<BattlefieldMap>) {
    let grid_color = Color::srgba(0.0, 0.8, 0.25, 0.18);
    let border_color = Color::srgba(0.0, 1.0, 0.35, 0.55);
    let axis_color = Color::srgba(0.0, 1.0, 0.35, 0.35);

    gizmos
        .grid_2d(
            Isometry2d::IDENTITY,
            map.cells,
            Vec2::splat(map.cell_size),
            grid_color,
        )
        .outer_edges();

    let size = map.cells.as_vec2() * map.cell_size;
    gizmos.rect_2d(Isometry2d::IDENTITY, size, border_color);

    // Center axes give the empty map a radar-screen feel and establish orientation.
    gizmos.line_2d(Vec2::new(-size.x / 2.0, 0.0), Vec2::new(size.x / 2.0, 0.0), axis_color);
    gizmos.line_2d(Vec2::new(0.0, -size.y / 2.0), Vec2::new(0.0, size.y / 2.0), axis_color);

}

fn draw_units(
    units: Query<(&BattlefieldPosition, Option<&Heading>, &Allegiance), With<Soldier>>,
    mut gizmos: Gizmos,
) {
    for (position, heading, allegiance) in &units {
        let color = match allegiance.side {
            Side::Blue => Color::srgb(0.0, 0.85, 1.0),
            Side::Red => Color::srgb(1.0, 0.15, 0.1),
        };

        let p = position.0;
        let radius = 7.0;
        gizmos.circle_2d(p, radius, color).resolution(24);

        if let Some(Heading(angle)) = heading {
            gizmos.line_2d(p, p + Vec2::from_angle(*angle) * radius, color);
        }
    }
}
