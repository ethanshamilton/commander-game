use super::RenderingSet;
use crate::GameState;
use crate::gameplay::combat::ResolvedShot;
use crate::gameplay::measurements::meters;
use bevy::prelude::*;

const UNIT_RADIUS_M: f32 = 0.7;

pub struct CombatRenderingPlugin;

impl Plugin for CombatRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (spawn_tracers_from_resolved_shots, draw_and_cleanup_tracers)
                .chain()
                .in_set(RenderingSet::Overlays)
                .run_if(in_state(GameState::ScenarioScreen)),
        );
    }
}

#[derive(Component, Debug, Clone, Copy)]
struct Tracer {
    start_m: Vec2,
    end_m: Vec2,
    speed_mps: f32,
    length_m: f32,
    elapsed_s: f32,
    hit: bool,
}

fn spawn_tracers_from_resolved_shots(
    mut commands: Commands,
    mut resolved_shots: MessageReader<ResolvedShot>,
) {
    for shot in resolved_shots.read() {
        let start_m = point_from_circle_border(
            shot.shooter_position_m,
            shot.impact_position_m,
            UNIT_RADIUS_M,
        )
        .unwrap_or(shot.shooter_position_m);

        commands.spawn(Tracer {
            start_m,
            end_m: shot.impact_position_m,
            speed_mps: shot.projectile_speed_mps,
            length_m: shot.tracer_length_m,
            elapsed_s: 0.0,
            hit: shot.hit,
        });
    }
}

fn draw_and_cleanup_tracers(
    mut commands: Commands,
    time: Res<Time>,
    mut tracers: Query<(Entity, &mut Tracer)>,
    mut gizmos: Gizmos,
) {
    for (entity, mut tracer) in &mut tracers {
        tracer.elapsed_s += time.delta_secs();

        let distance_m = tracer.start_m.distance(tracer.end_m);
        if distance_m <= f32::EPSILON {
            commands.entity(entity).despawn();
            continue;
        }

        let travelled_m = tracer.elapsed_s * tracer.speed_mps;
        let tip_t = (travelled_m / distance_m).clamp(0.0, 1.0);
        let tail_t = ((travelled_m - tracer.length_m) / distance_m).clamp(0.0, 1.0);
        let tail = tracer.start_m.lerp(tracer.end_m, tail_t).map(meters);
        let tip = tracer.start_m.lerp(tracer.end_m, tip_t).map(meters);
        let color = if tracer.hit {
            Color::srgb(1.0, 0.95, 0.55)
        } else {
            Color::srgb(1.0, 0.55, 0.15)
        };

        gizmos.line_2d(tail, tip, color);

        if tip_t >= 1.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn point_from_circle_border(center: Vec2, toward: Vec2, radius: f32) -> Option<Vec2> {
    let offset = toward - center;
    let distance = offset.length();

    if distance <= radius + f32::EPSILON {
        return None;
    }

    Some(center + offset / distance * radius)
}
