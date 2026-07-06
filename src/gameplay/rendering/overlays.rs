#![doc = include_str!("../../../docs/gameplay/rendering/overlays.md")]

use super::RenderingSet;
use crate::GameState;
use crate::actors::units::{Alive, Allegiance, Soldier};
use crate::actors::weapons::Weapon;
use crate::ai::perception::{
    AuditorySensor, Contact, ContactType, EyeHeight, PerceptionMemory, VisualSensor,
    has_line_of_sight,
};
use crate::gameplay::command::CommandForest;
use crate::gameplay::comms::{CommsGraph, VoiceComms};
use crate::gameplay::map::BattlefieldMap;
use crate::gameplay::measurements::meters;
use crate::gameplay::simulation::{SimulationClock, UnitOrder};
use crate::gameplay::spatial::{BattlefieldPosition, Heading};
use crate::intel::ReportedLifeStatus;
use crate::player::control::{PlayerControl, UnitIntelAccess};
use crate::player::knowledge::{
    CONTACT_RECENCY_TTL_TICKS, PlayerControlledUnit, PlayerTacticalKnowledge,
    REPORT_RECENCY_TTL_TICKS,
};
use crate::player::selection::SelectedUnit;
use bevy::prelude::*;

pub struct TacticalOverlayRenderingPlugin;

impl Plugin for TacticalOverlayRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                draw_selected_unit_sensor_cone,
                draw_selected_unit_weapon_range,
                draw_selected_unit_order,
                draw_selected_unit_contacts,
                draw_selected_unit_comms,
                draw_selected_unit_command_relations,
                draw_enemy_contact_boxes,
            )
                .in_set(RenderingSet::Overlays)
                .run_if(in_state(GameState::MissionScreen)),
        );
    }
}

const SENSOR_CONE_SEGMENTS: usize = 48;
const SENSOR_VISIBILITY_STEP_METERS: f32 = 2.0;
const WEAPON_RANGE_RING_ALPHA: f32 = 0.45;
const ORDER_DESTINATION_RADIUS: f32 = 4.0;
const CONTACT_BOX_SIZE: f32 = 24.0;
const UNIT_OVERLAY_RADIUS: f32 = 7.0;
const COMMAND_LINE_OFFSET: f32 = 5.0;
const COMMAND_ARROW_HEAD_LENGTH: f32 = 10.0;
const COMMAND_ARROW_HEAD_ANGLE: f32 = std::f32::consts::PI / 7.0;

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

    if allegiance.side == control.side
        && !knowledge.is_recently_reported(entity, clock.tick, REPORT_RECENCY_TTL_TICKS)
    {
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

    if let Some(left_endpoint) = left_endpoint
        && let Some(line_start) =
            point_from_circle_border(origin, left_endpoint, UNIT_OVERLAY_RADIUS)
    {
        gizmos.line_2d(line_start, left_endpoint, color);
    }

    if let Some(right_endpoint) = previous
        && let Some(line_start) =
            point_from_circle_border(origin, right_endpoint, UNIT_OVERLAY_RADIUS)
    {
        gizmos.line_2d(line_start, right_endpoint, color);
    }
}

fn draw_selected_unit_weapon_range(
    selected: Res<SelectedUnit>,
    control: Res<PlayerControl>,
    clock: Res<SimulationClock>,
    knowledge: Res<PlayerTacticalKnowledge>,
    units: Query<(&BattlefieldPosition, &Allegiance, &Weapon), With<Soldier>>,
    mut gizmos: Gizmos,
) {
    let Some(entity) = selected.entity else {
        return;
    };

    let Ok((position, allegiance, weapon)) = units.get(entity) else {
        return;
    };

    if allegiance.side != control.side
        || !knowledge.is_recently_reported(entity, clock.tick, REPORT_RECENCY_TTL_TICKS)
    {
        return;
    }

    let center = position.0.map(meters);
    let color = Color::srgba(1.0, 0.1, 0.1, WEAPON_RANGE_RING_ALPHA);

    gizmos
        .circle_2d(center, meters(weapon.max_range_m), color)
        .resolution(96);
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

    if !knowledge.is_recently_reported(entity, clock.tick, REPORT_RECENCY_TTL_TICKS) {
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

    if let Some(line_start) = point_from_circle_border(start, destination, UNIT_OVERLAY_RADIUS) {
        gizmos.line_2d(line_start, destination, color);
    }
    gizmos
        .circle_2d(destination, ORDER_DESTINATION_RADIUS, color)
        .resolution(16);
}

fn draw_selected_unit_contacts(
    selected: Res<SelectedUnit>,
    clock: Res<SimulationClock>,
    control: Res<PlayerControl>,
    knowledge: Res<PlayerTacticalKnowledge>,
    graph: Res<CommsGraph>,
    controlled: Query<Entity, (With<PlayerControlledUnit>, With<Alive>)>,
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

    let Ok(controlled_entity) = controlled.single() else {
        return;
    };

    if !graph.can_reach(controlled_entity, entity, control.side, |candidate| {
        units
            .get(candidate)
            .ok()
            .map(|(_, allegiance, _, _)| allegiance.side)
    }) {
        return;
    }

    let Ok((position, allegiance, memory, intel)) = units.get(entity) else {
        return;
    };

    if allegiance.side == control.side
        && !knowledge.is_recently_reported(entity, clock.tick, REPORT_RECENCY_TTL_TICKS)
    {
        return;
    }

    if allegiance.side != control.side && !intel.is_some_and(|intel| intel.reveal_contacts) {
        return;
    }

    let start = position.0.map(meters);

    for contact in latest_contacts_by_target(&memory.contacts) {
        let actively_tracked = contact.last_seen_tick == clock.tick;
        let endpoint_m = if actively_tracked {
            targets
                .get(contact.target)
                .map(|target_position| target_position.0)
                .unwrap_or(contact.last_seen_position_m)
        } else {
            contact.last_seen_position_m
        };
        let color = if contact.observed_life_status == ReportedLifeStatus::Dead {
            dead_contact_color(actively_tracked)
        } else {
            contact_color(contact.contact_type, actively_tracked)
        };

        let endpoint = endpoint_m.map(meters);
        if contact.contact_type == ContactType::Hostile {
            draw_line_between_unit_borders(&mut gizmos, start, endpoint, color);
        }
        gizmos.rect_2d(
            Isometry2d::from_translation(endpoint),
            Vec2::splat(CONTACT_BOX_SIZE),
            color,
        );
    }
}

fn latest_contacts_by_target(contacts: &[Contact]) -> Vec<&Contact> {
    let mut latest = std::collections::HashMap::new();

    for contact in contacts {
        latest
            .entry(contact.target)
            .and_modify(|existing: &mut &Contact| {
                if contact.last_seen_tick > existing.last_seen_tick {
                    *existing = contact;
                }
            })
            .or_insert(contact);
    }

    latest.into_values().collect()
}

fn draw_selected_unit_comms(
    selected: Res<SelectedUnit>,
    control: Res<PlayerControl>,
    clock: Res<SimulationClock>,
    knowledge: Res<PlayerTacticalKnowledge>,
    graph: Res<CommsGraph>,
    units: Query<
        (
            &BattlefieldPosition,
            &Allegiance,
            Option<&VoiceComms>,
            Option<&AuditorySensor>,
        ),
        With<Soldier>,
    >,
    mut gizmos: Gizmos,
) {
    let Some(selected_entity) = selected.entity else {
        return;
    };

    let Ok((selected_position, selected_allegiance, selected_voice, selected_auditory)) =
        units.get(selected_entity)
    else {
        return;
    };

    if selected_allegiance.side != control.side
        || !knowledge.is_recently_reported(selected_entity, clock.tick, REPORT_RECENCY_TTL_TICKS)
    {
        return;
    }

    let color = Color::srgba(0.78, 0.55, 1.0, 0.75);
    let selected_position = selected_position.0.map(meters);

    if let (Some(_voice), Some(auditory)) = (selected_voice, selected_auditory) {
        gizmos
            .circle_2d(selected_position, meters(auditory.range_m), color)
            .resolution(64);
    }

    let reachable = graph.reachable_from(selected_entity, control.side, |entity| {
        units
            .get(entity)
            .ok()
            .map(|(_, allegiance, _, _)| allegiance.side)
    });

    let mut drawn_edges = std::collections::HashSet::new();

    for source in &reachable {
        let Ok((source_position, _, _, _)) = units.get(*source) else {
            continue;
        };

        let Some(links) = graph.links_from(*source) else {
            continue;
        };

        for link in links {
            if !reachable.contains(&link.target)
                || drawn_edges.contains(&(link.target, *source))
                || !drawn_edges.insert((*source, link.target))
            {
                continue;
            }

            let Ok((target_position, _, _, _)) = units.get(link.target) else {
                continue;
            };

            draw_line_between_unit_borders(
                &mut gizmos,
                source_position.0.map(meters),
                target_position.0.map(meters),
                color,
            );
        }
    }
}

fn draw_selected_unit_command_relations(
    selected: Res<SelectedUnit>,
    control: Res<PlayerControl>,
    clock: Res<SimulationClock>,
    knowledge: Res<PlayerTacticalKnowledge>,
    command_forest: Res<CommandForest>,
    units: Query<&Allegiance, With<Soldier>>,
    mut gizmos: Gizmos,
) {
    let Some(selected_entity) = selected.entity else {
        return;
    };

    let Some(selected_known) =
        visible_known_unit(&knowledge, selected_entity, control.side, clock.tick)
    else {
        return;
    };

    let Ok(selected_allegiance) = units.get(selected_entity) else {
        return;
    };

    let color = side_color(selected_allegiance.side, 1.0);
    let selected_position = selected_known.last_known_position_m.map(meters);

    if let Some(superior) = command_forest.superior_of(selected_entity)
        && let Some(superior_known) =
            visible_known_unit(&knowledge, superior, control.side, clock.tick)
    {
        draw_arrow_2d(
            &mut gizmos,
            selected_position,
            superior_known.last_known_position_m.map(meters),
            color,
        );
    }

    for subordinate in command_forest.subordinates_of(selected_entity) {
        let Some(subordinate_known) =
            visible_known_unit(&knowledge, *subordinate, control.side, clock.tick)
        else {
            continue;
        };

        let Ok(subordinate_allegiance) = units.get(*subordinate) else {
            continue;
        };

        draw_arrow_2d(
            &mut gizmos,
            subordinate_known.last_known_position_m.map(meters),
            selected_position,
            side_color(subordinate_allegiance.side, 1.0),
        );
    }
}

fn visible_known_unit(
    knowledge: &PlayerTacticalKnowledge,
    entity: Entity,
    player_side: crate::actors::units::Side,
    tick: u64,
) -> Option<&crate::player::knowledge::KnownUnit> {
    let known = knowledge.get(entity)?;
    let visible = if known.side == player_side {
        tick.saturating_sub(known.last_reported_tick) <= REPORT_RECENCY_TTL_TICKS
    } else {
        tick.saturating_sub(known.last_observed_tick) <= CONTACT_RECENCY_TTL_TICKS
    };

    visible.then_some(known)
}

fn draw_arrow_2d(gizmos: &mut Gizmos, start: Vec2, end: Vec2, color: Color) {
    let offset = end - start;
    let length = offset.length();

    if length <= UNIT_OVERLAY_RADIUS * 2.0 + f32::EPSILON {
        return;
    }

    let direction = offset / length;
    let perpendicular = direction.perp();
    let line_offset = perpendicular * COMMAND_LINE_OFFSET;
    let Some(line_start) =
        point_from_circle_border(start, end, UNIT_OVERLAY_RADIUS).map(|p| p + line_offset)
    else {
        return;
    };
    let Some(line_end) =
        point_from_circle_border(end, start, UNIT_OVERLAY_RADIUS).map(|p| p + line_offset)
    else {
        return;
    };
    let line_length = line_start.distance(line_end);
    let arrow_head_length = COMMAND_ARROW_HEAD_LENGTH.min(line_length * 0.35);
    let left =
        Vec2::from_angle(direction.to_angle() + std::f32::consts::PI - COMMAND_ARROW_HEAD_ANGLE);
    let right =
        Vec2::from_angle(direction.to_angle() + std::f32::consts::PI + COMMAND_ARROW_HEAD_ANGLE);

    gizmos.line_2d(line_start, line_end, color);
    gizmos.line_2d(line_end, line_end + left * arrow_head_length, color);
    gizmos.line_2d(line_end, line_end + right * arrow_head_length, color);
}

fn draw_line_between_unit_borders(gizmos: &mut Gizmos, start: Vec2, end: Vec2, color: Color) {
    let Some(line_start) = point_from_circle_border(start, end, UNIT_OVERLAY_RADIUS) else {
        return;
    };
    let Some(line_end) = point_from_circle_border(end, start, UNIT_OVERLAY_RADIUS) else {
        return;
    };

    if line_start.distance_squared(line_end) <= f32::EPSILON {
        return;
    }

    gizmos.line_2d(line_start, line_end, color);
}

fn point_from_circle_border(center: Vec2, toward: Vec2, radius: f32) -> Option<Vec2> {
    let offset = toward - center;
    let distance = offset.length();

    if distance <= radius + f32::EPSILON {
        return None;
    }

    Some(center + offset / distance * radius)
}

fn side_color(side: crate::actors::units::Side, alpha: f32) -> Color {
    match side {
        crate::actors::units::Side::Blue => Color::srgba(0.0, 0.85, 1.0, alpha),
        crate::actors::units::Side::Red => Color::srgba(1.0, 0.15, 0.1, alpha),
    }
}

fn dead_contact_color(actively_tracked: bool) -> Color {
    let alpha = if actively_tracked { 1.0 } else { 0.55 };
    Color::srgba(0.55, 0.55, 0.55, alpha)
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
        let color = if unit.reported_life_status == ReportedLifeStatus::Dead {
            dead_contact_color(actively_tracked)
        } else {
            contact_color(ContactType::Hostile, actively_tracked)
        };

        gizmos.rect_2d(
            Isometry2d::from_translation(unit.last_known_position_m.map(meters)),
            Vec2::splat(CONTACT_BOX_SIZE),
            color,
        );
    }
}
