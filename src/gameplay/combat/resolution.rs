use super::CombatRng;
use super::components::{CombatOrder, CombatState};
use super::events::ResolvedShot;
use crate::actors::skills::Marksmanship;
use crate::actors::units::{Alive, Allegiance, Health, Soldier};
use crate::actors::weapons::Weapon;
use crate::ai::perception::{ContactKind, ContactType, PerceptionMemory};
use crate::gameplay::components::BattlefieldPosition;
use crate::gameplay::lifecycle::kill_unit;
use crate::gameplay::simulation::SimulationClock;
use bevy::prelude::*;
use rand::RngExt as _;
use rand::rngs::StdRng;

const UNIT_RADIUS_M: f32 = 0.7;

#[derive(Debug, Clone, Copy)]
struct ShotContext<'a> {
    weapon: &'a Weapon,
    distance_m: f32,
    marksmanship: f32,
    contact_confidence: f32,
}

pub fn resolve_combat(
    mut commands: Commands,
    clock: Res<SimulationClock>,
    mut rng: ResMut<CombatRng>,
    mut resolved_shots: MessageWriter<ResolvedShot>,
    mut shooters: Query<
        (
            Entity,
            &BattlefieldPosition,
            &Allegiance,
            &Weapon,
            &mut CombatState,
            &Marksmanship,
            &CombatOrder,
            &PerceptionMemory,
        ),
        (With<Soldier>, With<Alive>),
    >,
    mut targets: Query<
        (&BattlefieldPosition, &Allegiance, &mut Health),
        (With<Soldier>, With<Alive>),
    >,
) {
    for (
        shooter,
        shooter_position,
        shooter_allegiance,
        weapon,
        mut combat_state,
        marksmanship,
        combat_order,
        memory,
    ) in &mut shooters
    {
        let CombatOrder::FireAt { target } = *combat_order else {
            continue;
        };

        if clock.tick < combat_state.next_fire_tick {
            continue;
        }

        let Some(contact) = current_hostile_visual_contact(memory, target, clock.tick) else {
            continue;
        };

        let Ok((target_position, target_allegiance, mut target_health)) = targets.get_mut(target)
        else {
            continue;
        };

        if target == shooter || target_allegiance.side == shooter_allegiance.side {
            continue;
        }

        let distance_m = shooter_position.0.distance(target_position.0);
        if distance_m > weapon.max_range_m || distance_m <= f32::EPSILON {
            continue;
        }

        let p_hit = hit_probability(ShotContext {
            weapon,
            distance_m,
            marksmanship: marksmanship.value,
            contact_confidence: contact.confidence,
        });
        let roll = rng.0.random::<f32>();
        let hit = roll <= p_hit;
        let impact_position_m = if hit {
            point_from_circle_border(target_position.0, shooter_position.0, UNIT_RADIUS_M)
                .unwrap_or(target_position.0)
        } else {
            random_miss_endpoint(
                &mut rng.0,
                shooter_position.0,
                target_position.0,
                p_hit,
                roll,
            )
        };

        if hit {
            target_health.current = (target_health.current - weapon.damage).max(0);
            if target_health.current == 0 {
                kill_unit(&mut commands, target);
            }
        }

        resolved_shots.write(ResolvedShot {
            shooter,
            target,
            shooter_position_m: shooter_position.0,
            target_position_m: target_position.0,
            impact_position_m,
            hit,
            damage: weapon.damage,
            projectile_speed_mps: weapon.projectile_speed_mps,
            tracer_length_m: weapon.tracer_length_m,
        });

        combat_state.next_fire_tick = clock.tick + weapon.cooldown_ticks;
    }
}

fn current_hostile_visual_contact(
    memory: &PerceptionMemory,
    target: Entity,
    tick: u64,
) -> Option<&crate::ai::perception::Contact> {
    memory.contacts.iter().find(|contact| {
        contact.target == target
            && contact.last_seen_tick == tick
            && contact.kind == ContactKind::Visual
            && contact.contact_type == ContactType::Hostile
    })
}

fn hit_probability(ctx: ShotContext) -> f32 {
    let range_factor = range_modifier(
        ctx.distance_m,
        ctx.weapon.effective_range_m,
        ctx.weapon.max_range_m,
    );

    let p = ctx.weapon.base_accuracy * range_factor * ctx.marksmanship * ctx.contact_confidence;

    p.clamp(0.01, 0.95)
}

fn range_modifier(distance_m: f32, effective_range_m: f32, max_range_m: f32) -> f32 {
    if distance_m <= effective_range_m {
        return 1.0;
    }

    if distance_m >= max_range_m {
        return 0.25;
    }

    let t = (distance_m - effective_range_m) / (max_range_m - effective_range_m);
    1.0_f32.lerp(0.25, t)
}

fn random_miss_endpoint(
    rng: &mut StdRng,
    shooter_position_m: Vec2,
    target_position_m: Vec2,
    p_hit: f32,
    roll: f32,
) -> Vec2 {
    let offset = target_position_m - shooter_position_m;
    let distance_m = offset.length();

    if distance_m <= f32::EPSILON {
        return target_position_m;
    }

    let forward = offset / distance_m;
    let lateral = forward.perp();
    let miss_severity = ((roll - p_hit) / (1.0 - p_hit)).clamp(0.0, 1.0);
    let overshoot_m = rng.random_range(0.0..10.0) * miss_severity;
    let lateral_m = rng.random_range(-8.0..8.0) * miss_severity;

    target_position_m + forward * overshoot_m + lateral * lateral_m
}

fn point_from_circle_border(center: Vec2, toward: Vec2, radius: f32) -> Option<Vec2> {
    let offset = toward - center;
    let distance = offset.length();

    if distance <= radius + f32::EPSILON {
        return None;
    }

    Some(center + offset / distance * radius)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rifle() -> Weapon {
        Weapon::default_rifle()
    }

    fn p_at(distance_m: f32) -> f32 {
        hit_probability(ShotContext {
            weapon: &rifle(),
            distance_m,
            marksmanship: 1.0,
            contact_confidence: 1.0,
        })
    }

    #[test]
    fn hit_probability_never_increases_with_distance() {
        let mut previous = f32::INFINITY;
        for distance_m in [1.0, 35.0, 70.0, 71.0, 100.0, 139.0, 140.0, 200.0] {
            let p = p_at(distance_m);
            assert!(
                p <= previous,
                "p_hit rose from {previous} to {p} at {distance_m}m"
            );
            previous = p;
        }
    }

    #[test]
    fn range_modifier_is_continuous_at_both_knees() {
        let w = rifle();
        // no discontinuity where the lerp region meets the flat regions
        let eps = 0.01;
        let at_effective = range_modifier(w.effective_range_m, w.effective_range_m, w.max_range_m);
        let just_past = range_modifier(w.effective_range_m + eps, w.effective_range_m, w.max_range_m);
        assert!((at_effective - 1.0).abs() < 1e-6);
        assert!((just_past - 1.0).abs() < 1e-3);

        let at_max = range_modifier(w.max_range_m, w.effective_range_m, w.max_range_m);
        let just_before = range_modifier(w.max_range_m - eps, w.effective_range_m, w.max_range_m);
        assert!((at_max - 0.25).abs() < 1e-6);
        assert!((just_before - 0.25).abs() < 1e-3);
    }

    #[test]
    fn hit_probability_clamps_under_degenerate_inputs() {
        let superhuman = hit_probability(ShotContext {
            weapon: &rifle(),
            distance_m: 1.0,
            marksmanship: 50.0,
            contact_confidence: 1.0,
        });
        assert!(superhuman <= 0.95);

        let blind = hit_probability(ShotContext {
            weapon: &rifle(),
            distance_m: 200.0,
            marksmanship: 0.0,
            contact_confidence: 0.0,
        });
        assert!(blind >= 0.01);
    }

    #[test]
    fn miss_endpoints_never_land_on_the_target() {
        use rand::SeedableRng;
        let mut rng = StdRng::seed_from_u64(42);
        let shooter = Vec2::ZERO;
        let target = Vec2::new(50.0, 0.0);

        // near-miss rolls should land close; wild rolls land far, but a miss
        // with severity > 0 must displace away from the exact target point
        for roll in [0.51, 0.75, 0.99] {
            let p_hit = 0.5;
            let endpoint = random_miss_endpoint(&mut rng, shooter, target, p_hit, roll);
            assert!(
                endpoint.distance(target) > 0.0,
                "miss with roll {roll} landed exactly on target"
            );
        }
        // fully degenerate: shooter standing on target falls back to target pos
        let degenerate = random_miss_endpoint(&mut rng, target, target, 0.5, 0.9);
        assert_eq!(degenerate, target);
    }
}
