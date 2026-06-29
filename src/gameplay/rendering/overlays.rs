#![doc = include_str!("../../../docs/gameplay/rendering/overlays.md")]

use crate::GameState;
use crate::ai::perception::{
    ContactType, EyeHeight, PerceptionMemory, VisualSensor, has_line_of_sight,
};
use crate::gameplay::comms::{CommsLinks, VoiceComms};
use crate::gameplay::components::{BattlefieldPosition, Heading};
use crate::gameplay::map::BattlefieldMap;
use crate::gameplay::measurements::meters;
use crate::gameplay::simulation::{SimulationClock, UnitOrder};
use crate::player::control::{PlayerControl, UnitIntelAccess};
use crate::player::knowledge::{PlayerTacticalKnowledge, reachable_friendly_units};
use crate::player::selection::SelectedUnit;
use crate::units::{Allegiance, Soldier};
use bevy::prelude::*;

pub struct TacticalOverlayRenderingPlugin;

impl Plugin for TacticalOverlayRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                draw_selected_unit_sensor_cone,
                draw_selected_unit_order,
                draw_selected_unit_contacts,
                draw_selected_unit_comms,
                draw_enemy_contact_boxes,
            )
                .run_if(in_state(GameState::MissionScreen)),
        );
    }
}

const SENSOR_CONE_SEGMENTS: usize = 48;
const SENSOR_VISIBILITY_STEP_METERS: f32 = 2.0;
const ORDER_DESTINATION_RADIUS: f32 = 4.0;
const CONTACT_BOX_SIZE: f32 = 24.0;

fn draw_selected_unit_sensor_cone(
    selected: Res<SelectedUnit>,
    control: Res<PlayerControl>,
    clock: Res<SimulationClock>,
    knowledge: Res<PlayerTacticalKnowledge>,
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

    if allegiance.side == control.side && !knowledge.is_current(entity, clock.tick) {
        return;
    }

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

fn draw_selected_unit_order(
    selected: Res<SelectedUnit>,
    clock: Res<SimulationClock>,
    knowledge: Res<PlayerTacticalKnowledge>,
    units: Query<(&BattlefieldPosition, Option<&UnitOrder>), With<Soldier>>,
    mut gizmos: Gizmos,
) {
    let Some(entity) = selected.entity else {
        return;
    };

    if !knowledge.is_current(entity, clock.tick) {
        return;
    }

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
    knowledge: Res<PlayerTacticalKnowledge>,
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

    if allegiance.side == control.side && !knowledge.is_current(entity, clock.tick) {
        return;
    }

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
        let color = contact_color(contact.contact_type, actively_tracked);

        let endpoint = endpoint_m.map(meters);
        if contact.contact_type == ContactType::Hostile {
            gizmos.line_2d(start, endpoint, color);
        }
        gizmos.rect_2d(
            Isometry2d::from_translation(endpoint),
            Vec2::splat(CONTACT_BOX_SIZE),
            color,
        );
    }
}

fn draw_selected_unit_comms(
    selected: Res<SelectedUnit>,
    control: Res<PlayerControl>,
    clock: Res<SimulationClock>,
    knowledge: Res<PlayerTacticalKnowledge>,
    units: Query<
        (
            &BattlefieldPosition,
            &Allegiance,
            &CommsLinks,
            Option<&VoiceComms>,
        ),
        With<Soldier>,
    >,
    mut gizmos: Gizmos,
) {
    let Some(selected_entity) = selected.entity else {
        return;
    };

    let Ok((selected_position, selected_allegiance, _, selected_voice)) =
        units.get(selected_entity)
    else {
        return;
    };

    if selected_allegiance.side != control.side
        || !knowledge.is_current(selected_entity, clock.tick)
    {
        return;
    }

    let color = Color::srgba(0.78, 0.55, 1.0, 0.75);
    let selected_position = selected_position.0.map(meters);

    if let Some(voice) = selected_voice {
        gizmos
            .circle_2d(selected_position, meters(voice.range_m), color)
            .resolution(64);
    }

    let reachable = reachable_friendly_units(selected_entity, control.side, |entity| {
        let Ok((_, allegiance, comms, _)) = units.get(entity) else {
            return None;
        };
        Some((
            allegiance.side,
            comms.links.iter().map(|link| link.target).collect(),
        ))
    });

    let mut drawn_edges = std::collections::HashSet::new();

    for source in &reachable {
        let Ok((source_position, _, comms, _)) = units.get(*source) else {
            continue;
        };

        for link in &comms.links {
            if !reachable.contains(&link.target)
                || drawn_edges.contains(&(link.target, *source))
                || !drawn_edges.insert((*source, link.target))
            {
                continue;
            }

            let Ok((target_position, _, _, _)) = units.get(link.target) else {
                continue;
            };

            gizmos.line_2d(
                source_position.0.map(meters),
                target_position.0.map(meters),
                color,
            );
        }
    }
}

fn contact_color(contact_type: ContactType, actively_tracked: bool) -> Color {
    let alpha = if actively_tracked { 1.0 } else { 0.55 };

    match contact_type {
        ContactType::Friendly => Color::srgba(0.0, 0.85, 1.0, alpha),
        ContactType::Hostile => Color::srgba(1.0, 0.9, 0.0, alpha),
        ContactType::Neutral => Color::srgba(0.85, 0.85, 0.85, alpha),
        ContactType::Unknown => Color::srgba(1.0, 0.9, 0.0, alpha),
    }
}

fn draw_enemy_contact_boxes(
    clock: Res<SimulationClock>,
    control: Res<PlayerControl>,
    knowledge: Res<PlayerTacticalKnowledge>,
    mut gizmos: Gizmos,
) {
    for unit in knowledge
        .units
        .iter()
        .filter(|unit| unit.side != control.side)
    {
        let actively_tracked = unit.last_observed_tick == clock.tick;
        gizmos.rect_2d(
            Isometry2d::from_translation(unit.last_known_position_m.map(meters)),
            Vec2::splat(CONTACT_BOX_SIZE),
            contact_color(ContactType::Hostile, actively_tracked),
        );
    }
}
