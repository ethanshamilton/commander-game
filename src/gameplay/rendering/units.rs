#![doc = include_str!("../../../docs/gameplay/rendering/units.md")]

use super::RenderingSet;
use crate::GameState;
use crate::actors::units::{Side, Soldier};
use crate::gameplay::measurements::meters;
use crate::gameplay::simulation::SimulationClock;
use crate::gameplay::spatial::Heading;
use crate::intel::ReportedLifeStatus;
use crate::player::control::PlayerControl;
use crate::player::knowledge::{
    CONTACT_RECENCY_TTL_TICKS, PlayerControlledUnit, PlayerTacticalKnowledge,
    REPORT_RECENCY_TTL_TICKS,
};
use crate::player::selection::SelectedUnit;
use bevy::prelude::*;

pub struct UnitRenderingPlugin;

impl Plugin for UnitRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            draw_units
                .in_set(RenderingSet::Units)
                .run_if(in_state(GameState::ScenarioScreen)),
        );
    }
}

const CONTACT_BOX_SIZE: f32 = 24.0;
const SELECTED_UNIT_BOX_SIZE: f32 = 20.0;
const PLAYER_CONTROLLED_STAR_RADIUS: f32 = 5.0;
const PLAYER_CONTROLLED_STAR_OFFSET: Vec2 = Vec2::new(0.0, 16.0);

fn draw_units(
    selected: Res<SelectedUnit>,
    clock: Res<SimulationClock>,
    control: Res<PlayerControl>,
    knowledge: Res<PlayerTacticalKnowledge>,
    units: Query<(Entity, Option<&Heading>, Option<&PlayerControlledUnit>), With<Soldier>>,
    mut gizmos: Gizmos,
) {
    for known in &knowledge.units {
        let position = known.last_known_position_m.map(meters);
        let report_age = clock.tick.saturating_sub(known.last_reported_tick);
        let observation_age = clock.tick.saturating_sub(known.last_observed_tick);
        let is_friendly = known.side == control.side;
        let alpha = if is_friendly {
            age_alpha(report_age, REPORT_RECENCY_TTL_TICKS)
        } else {
            age_alpha(observation_age, CONTACT_RECENCY_TTL_TICKS)
        };

        let color = if known.reported_life_status == ReportedLifeStatus::Dead {
            Color::srgba(0.55, 0.55, 0.55, alpha)
        } else {
            match known.side {
                Side::Blue => Color::srgba(0.0, 0.85, 1.0, alpha),
                Side::Red => Color::srgba(1.0, 0.15, 0.1, alpha),
            }
        };

        gizmos.circle_2d(position, 7.0, color).resolution(24);

        // Hostile units are rendered as knowledge-backed glyphs: the circle is
        // the reported tactical picture, and the yellow box preserves the
        // contact-marker language used elsewhere.
        if !is_friendly {
            gizmos.rect_2d(
                Isometry2d::from_translation(position),
                Vec2::splat(CONTACT_BOX_SIZE),
                Color::srgba(1.0, 0.9, 0.0, alpha),
            );
        }

        if let Ok((_, heading, player_controlled)) = units.get(known.entity) {
            if player_controlled.is_some() {
                draw_player_controlled_star(&mut gizmos, position);
            }

            if let Some(Heading(angle)) = heading
                && (player_controlled.is_some()
                    || knowledge.is_recently_reported(
                        known.entity,
                        clock.tick,
                        REPORT_RECENCY_TTL_TICKS,
                    ))
            {
                gizmos.line_2d(position, position + Vec2::from_angle(*angle) * 7.0, color);
            }
        }

        if selected.entity == Some(known.entity) {
            gizmos.rect_2d(
                Isometry2d::from_translation(position),
                Vec2::splat(SELECTED_UNIT_BOX_SIZE),
                Color::WHITE,
            );
        }
    }
}

fn age_alpha(age_ticks: u64, fresh_ttl_ticks: u64) -> f32 {
    if age_ticks <= fresh_ttl_ticks {
        1.0
    } else {
        0.45
    }
}

fn draw_player_controlled_star(gizmos: &mut Gizmos, position: Vec2) {
    let position = position + PLAYER_CONTROLLED_STAR_OFFSET;
    let color = Color::srgb(1.0, 0.82, 0.18);
    let points = 5;
    let inner_radius = PLAYER_CONTROLLED_STAR_RADIUS * 0.45;
    let outer_radius = PLAYER_CONTROLLED_STAR_RADIUS;
    let step = std::f32::consts::TAU / (points * 2) as f32;
    let start_angle = std::f32::consts::FRAC_PI_2;

    let mut previous = None;
    let mut first = None;

    for i in 0..points * 2 {
        let radius = if i % 2 == 0 {
            outer_radius
        } else {
            inner_radius
        };
        let point = position + Vec2::from_angle(start_angle + i as f32 * step) * radius;

        if first.is_none() {
            first = Some(point);
        }

        if let Some(previous) = previous {
            gizmos.line_2d(previous, point, color);
        }

        previous = Some(point);
    }

    if let (Some(previous), Some(first)) = (previous, first) {
        gizmos.line_2d(previous, first, color);
    }
}
