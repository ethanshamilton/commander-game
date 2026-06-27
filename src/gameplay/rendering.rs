use crate::GameState;
use crate::ai::perception::{EyeHeight, VisualSensor, has_line_of_sight};
use crate::gameplay::components::{BattlefieldPosition, Heading};
use crate::gameplay::measurements::{meters, to_meters};
use crate::gameplay::terrain::{TerrainDefinition, TerrainHeight};
use crate::maps::MapDefinition;
use crate::player::selection::SelectedUnit;
use crate::units::{Allegiance, Side, Soldier};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;

pub struct GameplayRenderingPlugin;

impl Plugin for GameplayRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BattlefieldMap>().add_systems(
            Update,
            (
                pan_battlefield_camera,
                zoom_battlefield_camera,
                draw_battlefield_grid,
                draw_topography,
                draw_selected_unit_sensor_cone,
                draw_units,
            )
                .run_if(in_state(GameState::MissionScreen)),
        );
    }
}

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

const MIN_CAMERA_SCALE: f32 = 0.25;
const MAX_CAMERA_SCALE: f32 = 4.0;
const ZOOM_SENSITIVITY: f32 = 0.2;

fn pan_battlefield_camera(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut cameras: Query<(&mut Transform, &Projection), With<Camera2d>>,
) {
    if !mouse_buttons.pressed(MouseButton::Middle) || mouse_motion.delta == Vec2::ZERO {
        return;
    }

    for (mut transform, projection) in &mut cameras {
        let Projection::Orthographic(orthographic) = projection else {
            continue;
        };

        let pan_delta = mouse_motion.delta * orthographic.scale;
        transform.translation.x -= pan_delta.x;
        transform.translation.y += pan_delta.y;
    }
}

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
            orthographic.scale =
                (orthographic.scale * zoom_factor).clamp(MIN_CAMERA_SCALE, MAX_CAMERA_SCALE);
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

    let size = map.size_m.map(meters);
    gizmos.rect_2d(Isometry2d::IDENTITY, size, border_color);

    // Center axes give the empty map a radar-screen feel and establish orientation.
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

const TOPOGRAPHY_SAMPLE_SPACING_METERS: f32 = 5.0;
const CONTOUR_INTERVAL_METERS: f32 = 2.0;
const MAX_CONTOUR_HEIGHT_METERS: f32 = 20.0;

fn draw_topography(mut gizmos: Gizmos, map: Res<BattlefieldMap>) {
    let contour_color = Color::srgba(0.78, 0.78, 0.78, 0.45);
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

const SENSOR_CONE_SEGMENTS: usize = 48;
const SENSOR_VISIBILITY_STEP_METERS: f32 = 2.0;
const SELECTED_UNIT_BOX_SIZE: f32 = 20.0;

fn draw_selected_unit_sensor_cone(
    selected: Res<SelectedUnit>,
    map: Res<BattlefieldMap>,
    units: Query<(&BattlefieldPosition, &Heading, &VisualSensor, &EyeHeight), With<Soldier>>,
    mut gizmos: Gizmos,
) {
    let Some(entity) = selected.entity else {
        return;
    };

    let Ok((position, Heading(heading), sensor, eye_height)) = units.get(entity) else {
        return;
    };

    let origin = position.0;
    let origin_m = origin.map(to_meters);
    let half_fov = sensor.fov_radians / 2.0;
    let color = Color::srgba(1.0, 1.0, 1.0, 0.28);

    let left_angle = *heading - half_fov;
    let right_angle = *heading + half_fov;
    let mut previous = None;

    for segment in 0..=SENSOR_CONE_SEGMENTS {
        let t = segment as f32 / SENSOR_CONE_SEGMENTS as f32;
        let angle = left_angle.lerp(right_angle, t);
        let endpoint =
            visible_sensor_endpoint(&map, origin_m, angle, sensor.range_m, eye_height.height_m);

        gizmos.line_2d(origin, endpoint, color);

        if let Some(previous) = previous {
            gizmos.line_2d(previous, endpoint, color);
        }
        previous = Some(endpoint);
    }
}

fn visible_sensor_endpoint(
    map: &BattlefieldMap,
    origin_m: Vec2,
    angle: f32,
    max_range_m: f32,
    eye_height_m: f32,
) -> Vec2 {
    let direction = Vec2::from_angle(angle);
    let mut distance_m = SENSOR_VISIBILITY_STEP_METERS;
    let mut last_visible_m = origin_m;

    while distance_m <= max_range_m {
        let candidate_m = origin_m + direction * distance_m;

        if !is_inside_map(candidate_m, map.size_m)
            || !has_line_of_sight(
                &map.terrain,
                origin_m,
                eye_height_m,
                candidate_m,
                eye_height_m,
            )
        {
            break;
        }

        last_visible_m = candidate_m;
        distance_m += SENSOR_VISIBILITY_STEP_METERS;
    }

    last_visible_m.map(meters)
}

fn is_inside_map(position_m: Vec2, map_size_m: Vec2) -> bool {
    let half_size = map_size_m / 2.0;
    position_m.x >= -half_size.x
        && position_m.x <= half_size.x
        && position_m.y >= -half_size.y
        && position_m.y <= half_size.y
}

fn draw_units(
    selected: Res<SelectedUnit>,
    units: Query<(Entity, &BattlefieldPosition, Option<&Heading>, &Allegiance), With<Soldier>>,
    mut gizmos: Gizmos,
) {
    for (entity, position, heading, allegiance) in &units {
        let color = match allegiance.side {
            Side::Blue => Color::srgb(0.0, 0.85, 1.0),
            Side::Red => Color::srgb(1.0, 0.15, 0.1),
        };

        let p = position.0;
        let radius = 7.0;
        gizmos.circle_2d(p, radius, color).resolution(24);

        if selected.entity == Some(entity) {
            gizmos.rect_2d(
                Isometry2d::from_translation(p),
                Vec2::splat(SELECTED_UNIT_BOX_SIZE),
                Color::WHITE,
            );
        }

        if let Some(Heading(angle)) = heading {
            gizmos.line_2d(p, p + Vec2::from_angle(*angle) * radius, color);
        }
    }
}
