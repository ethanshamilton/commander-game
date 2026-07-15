#![doc = include_str!("../../docs/player/knowledge.md")]

use crate::GameState;
use crate::actors::units::{Alive, Allegiance, Side, Soldier};
use crate::ai::perception::{ContactType, PerceptionMemory};
use crate::gameplay::packets::{
    Address, ContactClaim, Inbox, Outbox, PacketIdAllocator, PacketPayload, SeenPackets,
    StatusClaim,
};
use crate::gameplay::simulation::{SIMULATION_TICK_HZ, SimulationClock, SimulationSet};
use crate::gameplay::spatial::BattlefieldPosition;
use crate::intel::ReportedLifeStatus;
use bevy::prelude::*;

pub struct PlayerKnowledgePlugin;

impl Plugin for PlayerKnowledgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerTacticalKnowledge>().add_systems(
            FixedUpdate,
            (consume_player_report_packets, author_report_packets)
                .chain()
                .in_set(SimulationSet::Reports)
                .run_if(in_state(GameState::ScenarioScreen)),
        );
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct PlayerControlledUnit;

pub const REPORT_INTERVAL_TICKS: u64 = SIMULATION_TICK_HZ as u64;
pub const CONTACT_REPORT_MAX_AGE_TICKS: u64 = REPORT_INTERVAL_TICKS;
pub const REPORT_RECENCY_TTL_TICKS: u64 = REPORT_INTERVAL_TICKS * 2;
pub const CONTACT_RECENCY_TTL_TICKS: u64 = CONTACT_REPORT_MAX_AGE_TICKS * 2;

#[derive(Component, Debug, Clone, Copy)]
pub struct ReportCadence {
    pub status_interval_ticks: u64,
    pub contact_interval_ticks: u64,
    pub last_status_report_tick: Option<u64>,
    pub last_contact_report_tick: Option<u64>,
}

impl Default for ReportCadence {
    fn default() -> Self {
        Self {
            status_interval_ticks: REPORT_INTERVAL_TICKS,
            contact_interval_ticks: REPORT_INTERVAL_TICKS,
            last_status_report_tick: None,
            last_contact_report_tick: None,
        }
    }
}

#[derive(Resource, Debug, Default)]
pub struct PlayerTacticalKnowledge {
    pub units: Vec<KnownUnit>,
}

impl PlayerTacticalKnowledge {
    pub fn get(&self, entity: Entity) -> Option<&KnownUnit> {
        self.units.iter().find(|unit| unit.entity == entity)
    }

    pub fn report_age_ticks(&self, entity: Entity, tick: u64) -> Option<u64> {
        self.get(entity)
            .map(|unit| tick.saturating_sub(unit.last_reported_tick))
    }

    pub fn is_recently_reported(&self, entity: Entity, tick: u64, ttl_ticks: u64) -> bool {
        self.report_age_ticks(entity, tick)
            .is_some_and(|age| age <= ttl_ticks)
    }

    fn upsert_report(&mut self, report: KnownUnit) {
        if let Some(existing) = self
            .units
            .iter_mut()
            .find(|existing| existing.entity == report.entity)
        {
            if report.last_observed_tick >= existing.last_observed_tick {
                *existing = report;
            } else if report.last_reported_tick > existing.last_reported_tick {
                existing.last_reported_tick = report.last_reported_tick;
            }
        } else {
            self.units.push(report);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct KnownUnit {
    pub entity: Entity,
    pub side: Side,
    pub reported_life_status: ReportedLifeStatus,
    pub last_known_position_m: Vec2,
    /// Tick when this unit/contact was physically observed by the reporting unit.
    pub last_observed_tick: u64,
    /// Tick when the player received or refreshed the report through comms.
    pub last_reported_tick: u64,
}

fn consume_player_report_packets(
    clock: Res<SimulationClock>,
    mut knowledge: ResMut<PlayerTacticalKnowledge>,
    mut controlled: Query<
        (
            Entity,
            &BattlefieldPosition,
            &Allegiance,
            Option<&PerceptionMemory>,
            &mut Inbox,
        ),
        (With<PlayerControlledUnit>, With<Alive>),
    >,
    target_units: Query<&Allegiance, With<Soldier>>,
) {
    let Ok((controlled_entity, position, allegiance, memory, mut inbox)) = controlled.single_mut()
    else {
        return;
    };

    // The player directly knows the state of the node they inhabit. This is not
    // a comms report; it is self-state.
    knowledge.upsert_report(KnownUnit {
        entity: controlled_entity,
        side: allegiance.side,
        reported_life_status: ReportedLifeStatus::Alive,
        last_known_position_m: position.0,
        last_observed_tick: clock.tick,
        last_reported_tick: clock.tick,
    });

    // The player also directly knows what their own unit currently perceives.
    if let Some(memory) = memory {
        for contact in &memory.contacts {
            if contact.last_seen_tick != clock.tick {
                continue;
            }

            let side = target_units
                .get(contact.target)
                .ok()
                .map(|allegiance| allegiance.side)
                .unwrap_or_else(|| side_from_contact_type(allegiance.side, contact.contact_type));

            knowledge.upsert_report(KnownUnit {
                entity: contact.target,
                side,
                reported_life_status: contact.observed_life_status,
                last_known_position_m: contact.last_seen_position_m,
                last_observed_tick: contact.last_seen_tick,
                last_reported_tick: clock.tick,
            });
        }
    }

    let mut retained = Vec::with_capacity(inbox.packets.len());
    for entry in inbox.packets.drain(..) {
        match entry.packet.payload {
            PacketPayload::StatusReport(claim) => {
                knowledge.upsert_report(KnownUnit {
                    entity: claim.subject,
                    side: claim.side,
                    reported_life_status: claim.life_status,
                    last_known_position_m: claim.position_m,
                    last_observed_tick: claim.observed_tick,
                    last_reported_tick: clock.tick,
                });
            }
            PacketPayload::ContactReport(claim) => {
                knowledge.upsert_report(KnownUnit {
                    entity: claim.subject,
                    side: claim.side,
                    reported_life_status: claim.life_status,
                    last_known_position_m: claim.position_m,
                    last_observed_tick: claim.observed_tick,
                    last_reported_tick: clock.tick,
                });
            }
            PacketPayload::OrderCommand(_) => retained.push(entry),
        }
    }

    inbox.packets = retained;
}

fn author_report_packets(
    clock: Res<SimulationClock>,
    mut ids: ResMut<PacketIdAllocator>,
    controlled: Query<(Entity, &Allegiance), (With<PlayerControlledUnit>, With<Alive>)>,
    mut reporters: Query<
        (
            Entity,
            &BattlefieldPosition,
            &Allegiance,
            Option<&PerceptionMemory>,
            &mut Outbox,
            &mut SeenPackets,
            &mut ReportCadence,
        ),
        (With<Soldier>, With<Alive>),
    >,
    target_units: Query<&Allegiance, With<Soldier>>,
) {
    let Ok((controlled_entity, controlled_allegiance)) = controlled.single() else {
        return;
    };

    for (entity, position, allegiance, memory, mut outbox, mut seen, mut cadence) in &mut reporters
    {
        if entity == controlled_entity || allegiance.side != controlled_allegiance.side {
            continue;
        }

        if should_report(
            clock.tick,
            cadence.last_status_report_tick,
            cadence.status_interval_ticks,
        ) {
            outbox.send(
                &mut seen,
                &mut ids,
                entity,
                Address::Direct(controlled_entity),
                clock.tick,
                PacketPayload::StatusReport(StatusClaim {
                    subject: entity,
                    side: allegiance.side,
                    position_m: position.0,
                    observed_tick: clock.tick,
                    life_status: ReportedLifeStatus::Alive,
                }),
            );
            cadence.last_status_report_tick = Some(clock.tick);
        }

        let Some(memory) = memory else {
            continue;
        };

        if !should_report(
            clock.tick,
            cadence.last_contact_report_tick,
            cadence.contact_interval_ticks,
        ) {
            continue;
        }

        let mut authored_contact_report = false;
        for contact in &memory.contacts {
            if clock.tick.saturating_sub(contact.last_seen_tick) > CONTACT_REPORT_MAX_AGE_TICKS {
                continue;
            }

            let side = target_units
                .get(contact.target)
                .ok()
                .map(|allegiance| allegiance.side)
                .unwrap_or_else(|| side_from_contact_type(allegiance.side, contact.contact_type));

            outbox.send(
                &mut seen,
                &mut ids,
                entity,
                Address::Direct(controlled_entity),
                clock.tick,
                PacketPayload::ContactReport(ContactClaim {
                    subject: contact.target,
                    side,
                    position_m: contact.last_seen_position_m,
                    observed_tick: contact.last_seen_tick,
                    life_status: contact.observed_life_status,
                    contact_type: contact.contact_type,
                }),
            );
            authored_contact_report = true;
        }

        if authored_contact_report {
            cadence.last_contact_report_tick = Some(clock.tick);
        }
    }
}

fn should_report(current_tick: u64, last_tick: Option<u64>, interval_ticks: u64) -> bool {
    last_tick.is_none_or(|last_tick| current_tick.saturating_sub(last_tick) >= interval_ticks)
}

fn side_from_contact_type(observer_side: Side, contact_type: ContactType) -> Side {
    match contact_type {
        ContactType::Friendly => observer_side,
        ContactType::Hostile => match observer_side {
            Side::Blue => Side::Red,
            Side::Red => Side::Blue,
        },
        ContactType::Neutral | ContactType::Unknown => observer_side,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::units::{Rank, Role};
    use crate::ai::perception::{Contact, ContactKind};
    use crate::gameplay::packets::{InboxEntry, OrderCommand};
    use crate::gameplay::simulation::UnitOrder;
    use bevy::ecs::system::RunSystemOnce;

    fn spawn_unit(world: &mut World, side: Side, position: Vec2) -> Entity {
        world
            .spawn((
                Soldier {
                    rank: Rank::Private,
                    role: Role::Rifleman,
                },
                Alive,
                Allegiance { side },
                BattlefieldPosition(position),
                Inbox::default(),
                Outbox::default(),
                SeenPackets::default(),
                ReportCadence::default(),
            ))
            .id()
    }

    #[test]
    fn stale_newly_received_report_updates_receipt_without_overwriting_fresher_observation() {
        let mut world = World::new();
        let unit = world.spawn_empty().id();
        let mut knowledge = PlayerTacticalKnowledge::default();

        knowledge.upsert_report(KnownUnit {
            entity: unit,
            side: Side::Blue,
            reported_life_status: ReportedLifeStatus::Alive,
            last_known_position_m: Vec2::new(10.0, 10.0),
            last_observed_tick: 20,
            last_reported_tick: 20,
        });

        knowledge.upsert_report(KnownUnit {
            entity: unit,
            side: Side::Blue,
            reported_life_status: ReportedLifeStatus::Alive,
            last_known_position_m: Vec2::new(1.0, 1.0),
            last_observed_tick: 10,
            last_reported_tick: 30,
        });

        let known = knowledge.get(unit).unwrap();
        assert_eq!(known.last_known_position_m, Vec2::new(10.0, 10.0));
        assert_eq!(known.last_observed_tick, 20);
        assert_eq!(known.last_reported_tick, 30);
    }

    #[test]
    fn player_consumes_status_report_packet_into_tactical_knowledge() {
        let mut world = World::new();
        world.insert_resource(SimulationClock {
            tick: 5,
            ..default()
        });
        world.insert_resource(PlayerTacticalKnowledge::default());
        let player = spawn_unit(&mut world, Side::Blue, Vec2::ZERO);
        let reporter = spawn_unit(&mut world, Side::Blue, Vec2::new(4.0, 5.0));
        world.entity_mut(player).insert(PlayerControlledUnit);
        world
            .get_mut::<Inbox>(player)
            .unwrap()
            .packets
            .push(InboxEntry {
                packet: crate::gameplay::packets::InfoPacket {
                    id: crate::gameplay::packets::PacketId(1),
                    origin: reporter,
                    address: Address::Direct(player),
                    created_tick: 4,
                    payload: PacketPayload::StatusReport(StatusClaim {
                        subject: reporter,
                        side: Side::Blue,
                        position_m: Vec2::new(4.0, 5.0),
                        observed_tick: 4,
                        life_status: ReportedLifeStatus::Alive,
                    }),
                },
                fresh: false,
            });

        world
            .run_system_once(consume_player_report_packets)
            .unwrap();

        let knowledge = world.resource::<PlayerTacticalKnowledge>();
        let known = knowledge.get(reporter).unwrap();
        assert_eq!(known.side, Side::Blue);
        assert_eq!(known.last_known_position_m, Vec2::new(4.0, 5.0));
        assert_eq!(known.last_observed_tick, 4);
        assert_eq!(known.last_reported_tick, 5);
        assert!(world.get::<Inbox>(player).unwrap().packets.is_empty());
    }

    #[test]
    fn player_consumes_contact_report_packet_but_retains_non_report_packets() {
        let mut world = World::new();
        world.insert_resource(SimulationClock {
            tick: 8,
            ..default()
        });
        world.insert_resource(PlayerTacticalKnowledge::default());
        let player = spawn_unit(&mut world, Side::Blue, Vec2::ZERO);
        let reporter = spawn_unit(&mut world, Side::Blue, Vec2::new(1.0, 1.0));
        let hostile = spawn_unit(&mut world, Side::Red, Vec2::new(9.0, 9.0));
        world.entity_mut(player).insert(PlayerControlledUnit);
        world.get_mut::<Inbox>(player).unwrap().packets.extend([
            InboxEntry {
                packet: crate::gameplay::packets::InfoPacket {
                    id: crate::gameplay::packets::PacketId(1),
                    origin: reporter,
                    address: Address::Direct(player),
                    created_tick: 7,
                    payload: PacketPayload::ContactReport(ContactClaim {
                        subject: hostile,
                        side: Side::Red,
                        position_m: Vec2::new(6.0, 7.0),
                        observed_tick: 7,
                        life_status: ReportedLifeStatus::Alive,
                        contact_type: ContactType::Hostile,
                    }),
                },
                fresh: false,
            },
            InboxEntry {
                packet: crate::gameplay::packets::InfoPacket {
                    id: crate::gameplay::packets::PacketId(2),
                    origin: player,
                    address: Address::Direct(player),
                    created_tick: 7,
                    payload: PacketPayload::OrderCommand(OrderCommand::Unit(UnitOrder::Hold)),
                },
                fresh: false,
            },
        ]);

        world
            .run_system_once(consume_player_report_packets)
            .unwrap();

        let knowledge = world.resource::<PlayerTacticalKnowledge>();
        let known = knowledge.get(hostile).unwrap();
        assert_eq!(known.side, Side::Red);
        assert_eq!(known.last_known_position_m, Vec2::new(6.0, 7.0));
        assert_eq!(known.last_observed_tick, 7);
        assert_eq!(known.last_reported_tick, 8);
        let inbox = world.get::<Inbox>(player).unwrap();
        assert_eq!(inbox.packets.len(), 1);
        assert!(matches!(
            inbox.packets[0].packet.payload,
            PacketPayload::OrderCommand(_)
        ));
    }

    #[test]
    fn friendly_units_author_status_and_fresh_contact_reports_to_player() {
        let mut world = World::new();
        world.insert_resource(SimulationClock {
            tick: 11,
            ..default()
        });
        world.insert_resource(PacketIdAllocator::default());
        let player = spawn_unit(&mut world, Side::Blue, Vec2::ZERO);
        let reporter = spawn_unit(&mut world, Side::Blue, Vec2::new(2.0, 3.0));
        let hostile = spawn_unit(&mut world, Side::Red, Vec2::new(9.0, 9.0));
        world.entity_mut(player).insert(PlayerControlledUnit);
        world.entity_mut(reporter).insert(PerceptionMemory {
            contacts: vec![Contact {
                target: hostile,
                last_seen_position_m: Vec2::new(8.0, 9.0),
                last_seen_time_s: 0.0,
                last_seen_tick: 11,
                confidence: 1.0,
                observed_life_status: ReportedLifeStatus::Alive,
                kind: ContactKind::Visual,
                contact_type: ContactType::Hostile,
            }],
        });

        world.run_system_once(author_report_packets).unwrap();

        let outbox = world.get::<Outbox>(reporter).unwrap();
        assert_eq!(outbox.packets.len(), 2);
        assert!(matches!(
            outbox.packets[0].payload,
            PacketPayload::StatusReport(_)
        ));
        assert!(matches!(
            outbox.packets[1].payload,
            PacketPayload::ContactReport(_)
        ));
        assert!(
            outbox
                .packets
                .iter()
                .all(|packet| packet.address == Address::Direct(player))
        );
    }

    #[test]
    fn report_cadence_suppresses_reports_before_interval() {
        let mut world = World::new();
        world.insert_resource(SimulationClock {
            tick: 15,
            ..default()
        });
        world.insert_resource(PacketIdAllocator::default());
        let player = spawn_unit(&mut world, Side::Blue, Vec2::ZERO);
        let reporter = spawn_unit(&mut world, Side::Blue, Vec2::new(2.0, 3.0));
        world.entity_mut(player).insert(PlayerControlledUnit);
        world.entity_mut(reporter).insert(ReportCadence {
            last_status_report_tick: Some(10),
            last_contact_report_tick: Some(10),
            ..default()
        });

        world.run_system_once(author_report_packets).unwrap();

        assert!(world.get::<Outbox>(reporter).unwrap().packets.is_empty());
    }

    #[test]
    fn report_cadence_allows_reports_after_interval() {
        let mut world = World::new();
        world.insert_resource(SimulationClock {
            tick: REPORT_INTERVAL_TICKS + 10,
            ..default()
        });
        world.insert_resource(PacketIdAllocator::default());
        let player = spawn_unit(&mut world, Side::Blue, Vec2::ZERO);
        let reporter = spawn_unit(&mut world, Side::Blue, Vec2::new(2.0, 3.0));
        world.entity_mut(player).insert(PlayerControlledUnit);
        world.entity_mut(reporter).insert(ReportCadence {
            last_status_report_tick: Some(10),
            ..default()
        });

        world.run_system_once(author_report_packets).unwrap();

        let outbox = world.get::<Outbox>(reporter).unwrap();
        assert_eq!(outbox.packets.len(), 1);
        assert!(matches!(
            outbox.packets[0].payload,
            PacketPayload::StatusReport(_)
        ));
    }

    #[test]
    fn contact_report_cadence_reports_recent_contacts_not_only_same_tick_contacts() {
        let mut world = World::new();
        let tick = 30;
        world.insert_resource(SimulationClock { tick, ..default() });
        world.insert_resource(PacketIdAllocator::default());
        let player = spawn_unit(&mut world, Side::Blue, Vec2::ZERO);
        let reporter = spawn_unit(&mut world, Side::Blue, Vec2::new(2.0, 3.0));
        let hostile = spawn_unit(&mut world, Side::Red, Vec2::new(9.0, 9.0));
        world.entity_mut(player).insert(PlayerControlledUnit);
        world.entity_mut(reporter).insert(ReportCadence {
            last_status_report_tick: Some(tick),
            last_contact_report_tick: Some(tick - REPORT_INTERVAL_TICKS),
            ..default()
        });
        world.entity_mut(reporter).insert(PerceptionMemory {
            contacts: vec![Contact {
                target: hostile,
                last_seen_position_m: Vec2::new(8.0, 9.0),
                last_seen_time_s: 0.0,
                last_seen_tick: tick - 1,
                confidence: 1.0,
                observed_life_status: ReportedLifeStatus::Alive,
                kind: ContactKind::Visual,
                contact_type: ContactType::Hostile,
            }],
        });

        world.run_system_once(author_report_packets).unwrap();

        let outbox = world.get::<Outbox>(reporter).unwrap();
        assert_eq!(outbox.packets.len(), 1);
        assert!(matches!(
            outbox.packets[0].payload,
            PacketPayload::ContactReport(_)
        ));
    }

    #[test]
    fn enemy_units_do_not_author_reports_to_player() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        world.insert_resource(PacketIdAllocator::default());
        let player = spawn_unit(&mut world, Side::Blue, Vec2::ZERO);
        let enemy = spawn_unit(&mut world, Side::Red, Vec2::new(2.0, 3.0));
        world.entity_mut(player).insert(PlayerControlledUnit);

        world.run_system_once(author_report_packets).unwrap();

        assert!(world.get::<Outbox>(enemy).unwrap().packets.is_empty());
    }
}
