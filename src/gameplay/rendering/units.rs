#![doc = include_str!("../../../docs/gameplay/rendering/units.md")]

use super::RenderingSet;
use crate::GameState;
use crate::actors::units::{Allegiance, Side, Soldier};
use crate::gameplay::components::Heading;
use crate::gameplay::measurements::meters;
use crate::gameplay::simulation::SimulationClock;
use crate::intel::ReportedLifeStatus;
use crate::player::control::PlayerControl;
use crate::player::knowledge::{PlayerControlledUnit, PlayerTacticalKnowledge};
use crate::player::selection::SelectedUnit;
use bevy::prelude::*;

pub struct UnitRenderingPlugin;

impl Plugin for UnitRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            draw_units
                .in_set(RenderingSet::Units)
                .run_if(in_state(GameState::MissionScreen)),
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
    units: Query<
        (
            Entity,
            Option<&Heading>,
            &Allegiance,
            Option<&PlayerControlledUnit>,
        ),
        With<Soldier>,
    >,
    mut gizmos: Gizmos,
) {
    for (entity, heading, allegiance, player_controlled) in &units {
        let Some(known) = knowledge.get(entity) else {
            continue;
        };

        let currently_reported = known.last_reported_tick == clock.tick;
        let actively_observed = known.last_observed_tick == clock.tick;
        let p = known.last_known_position_m.map(meters);

        if allegiance.side == control.side && !currently_reported {
            let color = if known.reported_life_status == ReportedLifeStatus::Dead {
                Color::srgba(0.55, 0.55, 0.55, 0.75)
            } else {
                Color::srgba(0.45, 0.85, 1.0, 0.75)
            };

            gizmos.rect_2d(
                Isometry2d::from_translation(p),
                Vec2::splat(CONTACT_BOX_SIZE),
                color,
            );
            continue;
        }

        if allegiance.side != control.side && !actively_observed {
            continue;
        }

        let color = if known.reported_life_status == ReportedLifeStatus::Dead {
            Color::srgb(0.55, 0.55, 0.55)
        } else {
            match allegiance.side {
                Side::Blue => Color::srgb(0.0, 0.85, 1.0),
                Side::Red => Color::srgb(1.0, 0.15, 0.1),
            }
        };

        let radius = 7.0;
        gizmos.circle_2d(p, radius, color).resolution(24);

        if player_controlled.is_some() {
            draw_player_controlled_star(&mut gizmos, p);
        }

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
