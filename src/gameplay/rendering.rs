use crate::GameState;
use crate::ai::perception::{EyeHeight, PerceptionMemory, VisualSensor, has_line_of_sight};
use crate::gameplay::components::{BattlefieldPosition, Heading};
use crate::gameplay::measurements::meters;
use crate::gameplay::simulation::{SimulationClock, UnitOrder};
use crate::gameplay::terrain::{TerrainDefinition, TerrainHeight};
use crate::maps::MapDefinition;
use crate::player::control::{PlayerControl, UnitIntelAccess};
use crate::player::selection::SelectedUnit;
use crate::units::{Allegiance, Side, Soldier};
use bevy::gizmos::config::{DefaultGizmoConfigGroup, GizmoConfigStore};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;

pub struct GameplayRenderingPlugin;

impl Plugin for GameplayRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BattlefieldMap>()
            .add_systems(Startup, configure_gizmos)
            .add_systems(
                Update,
                (
                    pan_battlefield_camera,
                    zoom_battlefield_camera,
                    draw_battlefield_grid,
                    draw_topography,
                    draw_selected_unit_sensor_cone,
                    draw_selected_unit_order,
                    draw_selected_unit_contacts,
                    draw_enemy_contact_boxes,
                    draw_units,
                )
                    .run_if(in_state(GameState::MissionScreen)),
            );
    }
}

fn configure_gizmos(mut config_store: ResMut<GizmoConfigStore>) {
    let (config, _) = config_store.config_mut::<DefaultGizmoConfigGroup>();
    config.line.width = 1.0;
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
    control: Res<PlayerControl>,
    map: Res<BattlefieldMap>,
    units: Query<
        (
            &BattlefieldPosition,
            &Heading,
            &VisualSensor,
            &EyeHeight,
            &Allegiance,
            Option<&UnitIntelAccess>,
        ),
        With<Soldier>,
    >,
    mut gizmos: Gizmos,
) {
    let Some(entity) = selected.entity else {
        return;
    };

    let Ok((position, Heading(heading), sensor, eye_height, allegiance, intel)) = units.get(entity)
    else {
        return;
    };

    if allegiance.side != control.side && !intel.is_some_and(|intel| intel.reveal_sensor_range) {
        return;
    }

    let origin_m = position.0;
    let half_fov = sensor.fov_radians / 2.0;
    let color = Color::srgba(0.45, 0.85, 1.0, 0.35);

    let left_angle = *heading - half_fov;
    let right_angle = *heading + half_fov;
    let origin = origin_m.map(meters);
    let mut previous = None;
    let mut left_endpoint = None;

    for segment in 0..=SENSOR_CONE_SEGMENTS {
        let t = segment as f32 / SENSOR_CONE_SEGMENTS as f32;
        let angle = left_angle.lerp(right_angle, t);
        let endpoint =
            visible_sensor_endpoint(&map, origin_m, angle, sensor.range_m, eye_height.height_m);

        if segment == 0 {
            left_endpoint = Some(endpoint);
        }

        if let Some(previous) = previous {
            gizmos.line_2d(previous, endpoint, color);
        }
        previous = Some(endpoint);
    }

    if let Some(left_endpoint) = left_endpoint {
        gizmos.line_2d(origin, left_endpoint, color);
    }

    if let Some(right_endpoint) = previous {
        gizmos.line_2d(origin, right_endpoint, color);
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

const ORDER_DESTINATION_RADIUS: f32 = 4.0;
const CONTACT_BOX_SIZE: f32 = 24.0;
const ACTIVE_CONTACT_COLOR: Color = Color::srgb(1.0, 0.9, 0.0);
const STALE_CONTACT_COLOR: Color = Color::srgb(0.55, 0.45, 0.0);

fn draw_selected_unit_order(
    selected: Res<SelectedUnit>,
    units: Query<(&BattlefieldPosition, Option<&UnitOrder>), With<Soldier>>,
    mut gizmos: Gizmos,
) {
    let Some(entity) = selected.entity else {
        return;
    };

    let Ok((position, order)) = units.get(entity) else {
        return;
    };

    let Some(UnitOrder::MoveTo { destination_m }) = order else {
        return;
    };

    let start = position.0.map(meters);
    let destination = destination_m.map(meters);
    let color = Color::WHITE;

    gizmos.line_2d(start, destination, color);
    gizmos
        .circle_2d(destination, ORDER_DESTINATION_RADIUS, color)
        .resolution(16);
}

fn draw_selected_unit_contacts(
    selected: Res<SelectedUnit>,
    clock: Res<SimulationClock>,
    control: Res<PlayerControl>,
    units: Query<
        (
            &BattlefieldPosition,
            &Allegiance,
            &PerceptionMemory,
            Option<&UnitIntelAccess>,
        ),
        With<Soldier>,
    >,
    targets: Query<&BattlefieldPosition, With<Soldier>>,
    mut gizmos: Gizmos,
) {
    let Some(entity) = selected.entity else {
        return;
    };

    let Ok((position, allegiance, memory, intel)) = units.get(entity) else {
        return;
    };

    if allegiance.side != control.side && !intel.is_some_and(|intel| intel.reveal_contacts) {
        return;
    }

    let start = position.0.map(meters);

    for contact in &memory.contacts {
        let actively_tracked = contact.last_seen_tick == clock.tick;
        let endpoint_m = if actively_tracked {
            targets
                .get(contact.target)
                .map(|target_position| target_position.0)
                .unwrap_or(contact.last_seen_position_m)
        } else {
            contact.last_seen_position_m
        };
        let color = contact_color(actively_tracked);

        let endpoint = endpoint_m.map(meters);
        gizmos.line_2d(start, endpoint, color);
        gizmos.rect_2d(
            Isometry2d::from_translation(endpoint),
            Vec2::splat(CONTACT_BOX_SIZE),
            color,
        );
    }
}

fn contact_color(actively_tracked: bool) -> Color {
    if actively_tracked {
        ACTIVE_CONTACT_COLOR
    } else {
        STALE_CONTACT_COLOR
    }
}

fn draw_enemy_contact_boxes(
    clock: Res<SimulationClock>,
    control: Res<PlayerControl>,
    observers: Query<(&Allegiance, &PerceptionMemory), With<Soldier>>,
    targets: Query<(&BattlefieldPosition, &Allegiance), With<Soldier>>,
    mut gizmos: Gizmos,
) {
    let mut contacts: Vec<(Entity, Vec2, bool, u64)> = Vec::new();

    for (observer_allegiance, memory) in &observers {
        if observer_allegiance.side != control.side {
            continue;
        }

        for contact in &memory.contacts {
            let Ok((target_position, target_allegiance)) = targets.get(contact.target) else {
                continue;
            };

            if target_allegiance.side == control.side {
                continue;
            }

            let actively_tracked = contact.last_seen_tick == clock.tick;
            let position_m = if actively_tracked {
                target_position.0
            } else {
                contact.last_seen_position_m
            };

            if let Some(existing) = contacts
                .iter_mut()
                .find(|(target, _, _, _)| *target == contact.target)
            {
                if actively_tracked || (!existing.2 && contact.last_seen_tick > existing.3) {
                    *existing = (
                        contact.target,
                        position_m,
                        actively_tracked,
                        contact.last_seen_tick,
                    );
                }
            } else {
                contacts.push((
                    contact.target,
                    position_m,
                    actively_tracked,
                    contact.last_seen_tick,
                ));
            }
        }
    }

    for (_, position_m, actively_tracked, _) in contacts {
        gizmos.rect_2d(
            Isometry2d::from_translation(position_m.map(meters)),
            Vec2::splat(CONTACT_BOX_SIZE),
            contact_color(actively_tracked),
        );
    }
}

fn draw_units(
    selected: Res<SelectedUnit>,
    clock: Res<SimulationClock>,
    control: Res<PlayerControl>,
    observers: Query<(&Allegiance, &PerceptionMemory), With<Soldier>>,
    units: Query<(Entity, &BattlefieldPosition, Option<&Heading>, &Allegiance), With<Soldier>>,
    mut gizmos: Gizmos,
) {
    for (entity, position, heading, allegiance) in &units {
        if allegiance.side != control.side {
            let actively_tracked = observers.iter().any(|(observer_allegiance, memory)| {
                observer_allegiance.side == control.side
                    && memory.contacts.iter().any(|contact| {
                        contact.target == entity && contact.last_seen_tick == clock.tick
                    })
            });

            if !actively_tracked {
                continue;
            }
        }

        let color = match allegiance.side {
            Side::Blue => Color::srgb(0.0, 0.85, 1.0),
            Side::Red => Color::srgb(1.0, 0.15, 0.1),
        };

        let p = position.0.map(meters);
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
