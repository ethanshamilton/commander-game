use super::state::{HostileBelief, PlannerState};
use crate::actors::units::{Health, Inventory};
use crate::ai::perception::{AuditorySensor, Contact, ContactKind, ContactType, PerceptionMemory};
use crate::gameplay::combat::ResolvedShot;
use crate::gameplay::simulation::{SimulationClock, UnitOrder};
use crate::gameplay::spatial::BattlefieldPosition;
use crate::intel::ReportedLifeStatus;
use bevy::prelude::*;
use std::cmp::Ordering;

/// Placeholder for richer threat memory. For now, recent nearby hostile contact
/// is a coarse suspicion signal; direct incoming-fire detection uses resolved
/// shot impacts supplied to synthesis.
const UNDER_FIRE_CONTACT_MAX_STALENESS_TICKS: u64 = 20;
const UNDER_FIRE_CONTACT_DISTANCE_M: f32 = 80.0;

pub fn synthesize_planner_state(
    clock: &SimulationClock,
    position: &BattlefieldPosition,
    health: &Health,
    inventory: &Inventory,
    memory: &PerceptionMemory,
    auditory_sensor: Option<&AuditorySensor>,
    current_order: Option<&UnitOrder>,
    recent_shots: &[ResolvedShot],
) -> PlannerState {
    let nearest_hostile = nearest_hostile_belief(position.0, memory);
    let under_fire = under_fire_from_impacts(position.0, auditory_sensor, recent_shots)
        || under_fire_from_contacts(clock.tick, position.0, memory);

    PlannerState {
        position_m: position.0,
        health_frac: health_fraction(health),
        has_ammo: inventory.has_ammo(),
        nearest_hostile,
        under_fire,
        has_move_target: matches!(current_order, Some(UnitOrder::MoveTo { .. })),
        tick: clock.tick,
    }
}

fn health_fraction(health: &Health) -> f32 {
    if health.max <= 0 {
        return 0.0;
    }

    (health.current as f32 / health.max as f32).clamp(0.0, 1.0)
}

fn nearest_hostile_belief(position_m: Vec2, memory: &PerceptionMemory) -> Option<HostileBelief> {
    let mut best_per_target: Vec<&Contact> = Vec::new();

    for contact in memory.contacts.iter().filter(|contact| {
        contact.contact_type == ContactType::Hostile
            && contact.observed_life_status == ReportedLifeStatus::Alive
            && contact.confidence > 0.0
    }) {
        if let Some(existing) = best_per_target
            .iter_mut()
            .find(|existing| existing.target == contact.target)
        {
            if contact_observation_cmp(contact, existing).is_gt() {
                *existing = contact;
            }
        } else {
            best_per_target.push(contact);
        }
    }

    best_per_target
        .into_iter()
        .min_by(|a, b| {
            let a_distance = position_m.distance_squared(a.last_seen_position_m);
            let b_distance = position_m.distance_squared(b.last_seen_position_m);
            a_distance.total_cmp(&b_distance)
        })
        .map(|contact| HostileBelief {
            entity: contact.target,
            position_m: contact.last_seen_position_m,
            confidence: contact.confidence,
            last_seen_tick: contact.last_seen_tick,
            kind: contact.kind,
        })
}

fn contact_observation_cmp(a: &Contact, b: &Contact) -> Ordering {
    a.last_seen_tick
        .cmp(&b.last_seen_tick)
        .then_with(|| a.confidence.total_cmp(&b.confidence))
        .then_with(|| contact_kind_priority(a.kind).cmp(&contact_kind_priority(b.kind)))
}

fn contact_kind_priority(kind: ContactKind) -> u8 {
    match kind {
        ContactKind::Visual => 4,
        ContactKind::Auditory => 3,
        ContactKind::Radar => 2,
        ContactKind::Unknown => 1,
    }
}

fn under_fire_from_impacts(
    position_m: Vec2,
    auditory_sensor: Option<&AuditorySensor>,
    recent_shots: &[ResolvedShot],
) -> bool {
    let Some(auditory_sensor) = auditory_sensor else {
        return false;
    };

    let range_sq = auditory_sensor.range_m * auditory_sensor.range_m;
    recent_shots
        .iter()
        .any(|shot| position_m.distance_squared(shot.impact_position_m) <= range_sq)
}

fn under_fire_from_contacts(tick: u64, position_m: Vec2, memory: &PerceptionMemory) -> bool {
    memory.contacts.iter().any(|contact| {
        contact.contact_type == ContactType::Hostile
            && contact.observed_life_status == ReportedLifeStatus::Alive
            && tick.saturating_sub(contact.last_seen_tick) <= UNDER_FIRE_CONTACT_MAX_STALENESS_TICKS
            && position_m.distance_squared(contact.last_seen_position_m)
                <= UNDER_FIRE_CONTACT_DISTANCE_M * UNDER_FIRE_CONTACT_DISTANCE_M
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::units::{Item, ItemKind};
    use bevy::ecs::world::World;

    fn inventory_with_ammo(count: u32) -> Inventory {
        Inventory {
            items: vec![Item {
                kind: ItemKind::Ammo,
                count,
            }],
        }
    }

    fn contact(
        target: Entity,
        position_m: Vec2,
        tick: u64,
        confidence: f32,
        kind: ContactKind,
        contact_type: ContactType,
        life: ReportedLifeStatus,
    ) -> Contact {
        Contact {
            target,
            last_seen_position_m: position_m,
            last_seen_time_s: 0.0,
            last_seen_tick: tick,
            confidence,
            observed_life_status: life,
            kind,
            contact_type,
        }
    }

    fn shot_at(impact_position_m: Vec2) -> ResolvedShot {
        let mut world = World::new();
        let shooter = world.spawn_empty().id();
        let target = world.spawn_empty().id();
        ResolvedShot {
            shooter,
            target,
            shooter_position_m: Vec2::ZERO,
            target_position_m: Vec2::ZERO,
            impact_position_m,
            hit: false,
            damage: 0,
            projectile_speed_mps: 700.0,
            tracer_length_m: 8.0,
        }
    }

    #[test]
    fn health_fraction_is_clamped() {
        assert_eq!(
            health_fraction(&Health {
                current: 150,
                max: 100
            }),
            1.0
        );
        assert_eq!(
            health_fraction(&Health {
                current: -10,
                max: 100
            }),
            0.0
        );
        assert_eq!(
            health_fraction(&Health {
                current: 10,
                max: 0
            }),
            0.0
        );
    }

    #[test]
    fn ammo_maps_from_inventory() {
        let clock = SimulationClock::default();
        let position = BattlefieldPosition(Vec2::ZERO);
        let memory = PerceptionMemory::default();
        let health = Health {
            current: 100,
            max: 100,
        };

        let empty = synthesize_planner_state(
            &clock,
            &position,
            &health,
            &inventory_with_ammo(0),
            &memory,
            None,
            None,
            &[],
        );
        let loaded = synthesize_planner_state(
            &clock,
            &position,
            &health,
            &inventory_with_ammo(1),
            &memory,
            None,
            None,
            &[],
        );

        assert!(!empty.has_ammo);
        assert!(loaded.has_ammo);
    }

    #[test]
    fn nearest_hostile_ignores_friendlies_dead_and_zero_confidence() {
        let mut world = World::new();
        let friendly = world.spawn_empty().id();
        let dead_hostile = world.spawn_empty().id();
        let zero_confidence = world.spawn_empty().id();
        let alive_hostile = world.spawn_empty().id();
        let memory = PerceptionMemory {
            contacts: vec![
                contact(
                    friendly,
                    Vec2::new(1.0, 0.0),
                    1,
                    1.0,
                    ContactKind::Visual,
                    ContactType::Friendly,
                    ReportedLifeStatus::Alive,
                ),
                contact(
                    dead_hostile,
                    Vec2::new(2.0, 0.0),
                    1,
                    1.0,
                    ContactKind::Visual,
                    ContactType::Hostile,
                    ReportedLifeStatus::Dead,
                ),
                contact(
                    zero_confidence,
                    Vec2::new(3.0, 0.0),
                    1,
                    0.0,
                    ContactKind::Visual,
                    ContactType::Hostile,
                    ReportedLifeStatus::Alive,
                ),
                contact(
                    alive_hostile,
                    Vec2::new(4.0, 0.0),
                    1,
                    1.0,
                    ContactKind::Visual,
                    ContactType::Hostile,
                    ReportedLifeStatus::Alive,
                ),
            ],
        };

        let belief = nearest_hostile_belief(Vec2::ZERO, &memory).unwrap();

        assert_eq!(belief.entity, alive_hostile);
    }

    #[test]
    fn nearest_hostile_chooses_nearest_after_deduping_targets() {
        let mut world = World::new();
        let far = world.spawn_empty().id();
        let near = world.spawn_empty().id();
        let memory = PerceptionMemory {
            contacts: vec![
                contact(
                    far,
                    Vec2::new(20.0, 0.0),
                    1,
                    1.0,
                    ContactKind::Visual,
                    ContactType::Hostile,
                    ReportedLifeStatus::Alive,
                ),
                contact(
                    near,
                    Vec2::new(5.0, 0.0),
                    1,
                    1.0,
                    ContactKind::Visual,
                    ContactType::Hostile,
                    ReportedLifeStatus::Alive,
                ),
            ],
        };

        let belief = nearest_hostile_belief(Vec2::ZERO, &memory).unwrap();

        assert_eq!(belief.entity, near);
    }

    #[test]
    fn newer_contact_wins_for_same_target() {
        let mut world = World::new();
        let target = world.spawn_empty().id();
        let memory = PerceptionMemory {
            contacts: vec![
                contact(
                    target,
                    Vec2::new(100.0, 0.0),
                    1,
                    1.0,
                    ContactKind::Visual,
                    ContactType::Hostile,
                    ReportedLifeStatus::Alive,
                ),
                contact(
                    target,
                    Vec2::new(10.0, 0.0),
                    2,
                    0.5,
                    ContactKind::Auditory,
                    ContactType::Hostile,
                    ReportedLifeStatus::Alive,
                ),
            ],
        };

        let belief = nearest_hostile_belief(Vec2::ZERO, &memory).unwrap();

        assert_eq!(belief.position_m, Vec2::new(10.0, 0.0));
        assert_eq!(belief.kind, ContactKind::Auditory);
    }

    #[test]
    fn visual_wins_contact_tie_over_auditory() {
        let mut world = World::new();
        let target = world.spawn_empty().id();
        let memory = PerceptionMemory {
            contacts: vec![
                contact(
                    target,
                    Vec2::new(20.0, 0.0),
                    1,
                    0.8,
                    ContactKind::Auditory,
                    ContactType::Hostile,
                    ReportedLifeStatus::Alive,
                ),
                contact(
                    target,
                    Vec2::new(10.0, 0.0),
                    1,
                    0.8,
                    ContactKind::Visual,
                    ContactType::Hostile,
                    ReportedLifeStatus::Alive,
                ),
            ],
        };

        let belief = nearest_hostile_belief(Vec2::ZERO, &memory).unwrap();

        assert_eq!(belief.position_m, Vec2::new(10.0, 0.0));
        assert_eq!(belief.kind, ContactKind::Visual);
    }

    #[test]
    fn under_fire_from_shot_impacts_in_auditory_range() {
        let sensor = AuditorySensor { range_m: 40.0 };

        assert!(under_fire_from_impacts(
            Vec2::ZERO,
            Some(&sensor),
            &[shot_at(Vec2::new(30.0, 0.0))],
        ));
        assert!(!under_fire_from_impacts(
            Vec2::ZERO,
            Some(&sensor),
            &[shot_at(Vec2::new(50.0, 0.0))],
        ));
        assert!(!under_fire_from_impacts(
            Vec2::ZERO,
            None,
            &[shot_at(Vec2::new(1.0, 0.0))],
        ));
    }

    #[test]
    fn under_fire_from_fresh_near_hostile_contact_fallback() {
        let mut world = World::new();
        let target = world.spawn_empty().id();
        let fresh_near = PerceptionMemory {
            contacts: vec![contact(
                target,
                Vec2::new(10.0, 0.0),
                90,
                1.0,
                ContactKind::Visual,
                ContactType::Hostile,
                ReportedLifeStatus::Alive,
            )],
        };
        let stale = PerceptionMemory {
            contacts: vec![contact(
                target,
                Vec2::new(10.0, 0.0),
                1,
                1.0,
                ContactKind::Visual,
                ContactType::Hostile,
                ReportedLifeStatus::Alive,
            )],
        };
        let far = PerceptionMemory {
            contacts: vec![contact(
                target,
                Vec2::new(100.0, 0.0),
                90,
                1.0,
                ContactKind::Visual,
                ContactType::Hostile,
                ReportedLifeStatus::Alive,
            )],
        };

        assert!(under_fire_from_contacts(100, Vec2::ZERO, &fresh_near));
        assert!(!under_fire_from_contacts(100, Vec2::ZERO, &stale));
        assert!(!under_fire_from_contacts(100, Vec2::ZERO, &far));
    }

    #[test]
    fn move_target_comes_from_current_order() {
        let clock = SimulationClock::default();
        let position = BattlefieldPosition(Vec2::ZERO);
        let memory = PerceptionMemory::default();
        let health = Health {
            current: 100,
            max: 100,
        };
        let inventory = inventory_with_ammo(1);

        let moving = synthesize_planner_state(
            &clock,
            &position,
            &health,
            &inventory,
            &memory,
            None,
            Some(&UnitOrder::MoveTo {
                destination_m: Vec2::new(1.0, 0.0),
            }),
            &[],
        );
        let holding = synthesize_planner_state(
            &clock,
            &position,
            &health,
            &inventory,
            &memory,
            None,
            Some(&UnitOrder::Hold),
            &[],
        );
        let no_order = synthesize_planner_state(
            &clock,
            &position,
            &health,
            &inventory,
            &memory,
            None,
            None,
            &[],
        );

        assert!(moving.has_move_target);
        assert!(!holding.has_move_target);
        assert!(!no_order.has_move_target);
    }

    #[test]
    fn headless_component_compatibility_synthesis() {
        let mut world = World::new();
        let hostile = world.spawn_empty().id();
        let unit = world
            .spawn((
                BattlefieldPosition(Vec2::new(1.0, 2.0)),
                Health {
                    current: 50,
                    max: 100,
                },
                inventory_with_ammo(5),
                AuditorySensor { range_m: 40.0 },
                UnitOrder::MoveTo {
                    destination_m: Vec2::new(3.0, 4.0),
                },
                PerceptionMemory {
                    contacts: vec![contact(
                        hostile,
                        Vec2::new(8.0, 2.0),
                        11,
                        0.9,
                        ContactKind::Visual,
                        ContactType::Hostile,
                        ReportedLifeStatus::Alive,
                    )],
                },
            ))
            .id();
        let clock = SimulationClock {
            tick: 11,
            ..default()
        };

        let position = world.get::<BattlefieldPosition>(unit).unwrap();
        let health = world.get::<Health>(unit).unwrap();
        let inventory = world.get::<Inventory>(unit).unwrap();
        let memory = world.get::<PerceptionMemory>(unit).unwrap();
        let auditory = world.get::<AuditorySensor>(unit);
        let order = world.get::<UnitOrder>(unit);

        let state = synthesize_planner_state(
            &clock,
            position,
            health,
            inventory,
            memory,
            auditory,
            order,
            &[shot_at(Vec2::new(2.0, 2.0))],
        );

        assert_eq!(state.position_m, Vec2::new(1.0, 2.0));
        assert_eq!(state.health_frac, 0.5);
        assert!(state.has_ammo);
        assert_eq!(state.nearest_hostile.unwrap().entity, hostile);
        assert!(state.under_fire);
        assert!(state.has_move_target);
        assert_eq!(state.tick, 11);
    }
}
