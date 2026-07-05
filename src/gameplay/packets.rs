#![doc = include_str!("../../docs/gameplay/packets.md")]
#![allow(dead_code)]

use crate::actors::units::{Alive, Soldier};
use crate::ai::perception::ContactType;
use crate::gameplay::comms::{CommsGraph, update_comms_graph};
use crate::gameplay::simulation::{SimulationClock, SimulationSet};
use crate::intel::ReportedLifeStatus;
use bevy::prelude::*;
use std::collections::HashSet;

pub const INBOX_TTL_TICKS: u64 = 600;

pub struct PacketsPlugin;

impl Plugin for PacketsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PacketIdAllocator>().add_systems(
            FixedUpdate,
            (prune_stale_inbox_packets, deliver_packets, relay_packets)
                .chain()
                .in_set(SimulationSet::Comms)
                .after(update_comms_graph),
        );
    }
}

/// Globally unique packet identifier, allocated from [`PacketIdAllocator`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PacketId(pub u64);

#[derive(Resource, Debug, Default)]
pub struct PacketIdAllocator {
    next: u64,
}

impl PacketIdAllocator {
    pub fn allocate(&mut self) -> PacketId {
        let id = PacketId(self.next);
        self.next += 1;
        id
    }
}

/// Who a packet is for.
///
/// Voice remains physically broadcast: delivery lets all direct neighbors hear
/// a packet, then later relay/consumer systems use this address to decide what
/// to do with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Address {
    Direct(Entity),
    Broadcast,
}

/// A unit of transmitted information.
///
/// Packets are immutable after creation and relayed verbatim. `origin` is the
/// original author and is never rewritten by relays.
#[derive(Debug, Clone, PartialEq)]
pub struct InfoPacket {
    pub id: PacketId,
    pub origin: Entity,
    pub address: Address,
    pub created_tick: u64,
    pub payload: PacketPayload,
}

/// Typed packet contents.
///
/// Payloads should contain belief snapshots/claims, not fresh ground truth.
#[derive(Debug, Clone, PartialEq)]
pub enum PacketPayload {
    ContactReport(ContactClaim),
}

/// Snapshot of a contact belief safe to transmit through comms.
#[derive(Debug, Clone, PartialEq)]
pub struct ContactClaim {
    pub subject: Entity,
    pub position_m: Vec2,
    pub observed_tick: u64,
    pub life_status: ReportedLifeStatus,
    pub contact_type: ContactType,
}

/// Packets this unit wants to transmit on a future comms pass.
#[derive(Component, Debug, Default)]
pub struct Outbox {
    pub packets: Vec<InfoPacket>,
}

impl Outbox {
    pub fn send(
        &mut self,
        seen: &mut SeenPackets,
        ids: &mut PacketIdAllocator,
        origin: Entity,
        address: Address,
        created_tick: u64,
        payload: PacketPayload,
    ) -> PacketId {
        let id = ids.allocate();
        seen.ids.insert(id);
        self.packets.push(InfoPacket {
            id,
            origin,
            address,
            created_tick,
            payload,
        });
        id
    }
}

/// Packets this unit has heard and may consume.
#[derive(Component, Debug, Default)]
pub struct Inbox {
    pub packets: Vec<InboxEntry>,
}

/// A received packet plus relay-processing state.
#[derive(Debug, Clone, PartialEq)]
pub struct InboxEntry {
    pub packet: InfoPacket,
    /// Fresh entries have not yet been considered by relay doctrine.
    pub fresh: bool,
}

/// Packet IDs this unit has already heard.
///
/// This prevents duplicate receipt and relay loops. V1 leaves pruning to a
/// later delivery/TTL pass.
#[derive(Component, Debug, Default)]
pub struct SeenPackets {
    pub ids: HashSet<PacketId>,
}

/// Drop inbox entries whose utterance has become too old to consume.
///
/// `SeenPackets` is intentionally not pruned here: if an old utterance matters,
/// the sender should re-issue it as a new packet with a new ID.
fn prune_stale_inbox_packets(
    clock: Res<SimulationClock>,
    mut inboxes: Query<&mut Inbox, (With<Soldier>, With<Alive>)>,
) {
    for mut inbox in &mut inboxes {
        prune_inbox(&mut inbox, clock.tick);
    }
}

fn prune_inbox(inbox: &mut Inbox, current_tick: u64) {
    inbox
        .packets
        .retain(|entry| current_tick.saturating_sub(entry.packet.created_tick) <= INBOX_TTL_TICKS);
}

/// Drain outboxes and deliver each packet to every direct comms neighbor that
/// has not already heard that packet ID.
fn deliver_packets(
    graph: Res<CommsGraph>,
    mut outboxes: Query<(Entity, &mut Outbox), (With<Soldier>, With<Alive>)>,
    mut receivers: Query<(&mut Inbox, &mut SeenPackets), (With<Soldier>, With<Alive>)>,
) {
    let mut sends = Vec::new();
    for (sender, mut outbox) in &mut outboxes {
        sends.extend(outbox.packets.drain(..).map(|packet| (sender, packet)));
    }

    let planned = plan_deliveries(
        &sends,
        |sender| {
            graph
                .links_from(sender)
                .map(|links| links.iter().map(|link| link.target).collect())
                .unwrap_or_default()
        },
        |receiver, packet_id| {
            receivers
                .get(receiver)
                .ok()
                .is_some_and(|(_, seen)| seen.ids.contains(&packet_id))
        },
    );

    for (receiver, packet) in planned {
        let Ok((mut inbox, mut seen)) = receivers.get_mut(receiver) else {
            continue;
        };

        if !seen.ids.insert(packet.id) {
            continue;
        }

        inbox.packets.push(InboxEntry {
            packet,
            fresh: true,
        });
    }
}

/// Plan one-hop packet deliveries for direct comms neighbors.
///
/// This is intentionally transport-level only: address does not filter physical
/// hearing. Consumers/relay doctrine interpret the address later.
fn plan_deliveries(
    sends: &[(Entity, InfoPacket)],
    mut links_from: impl FnMut(Entity) -> Vec<Entity>,
    mut already_seen: impl FnMut(Entity, PacketId) -> bool,
) -> Vec<(Entity, InfoPacket)> {
    let mut deliveries = Vec::new();

    for (sender, packet) in sends {
        for receiver in links_from(*sender) {
            if already_seen(receiver, packet.id) {
                continue;
            }

            deliveries.push((receiver, packet.clone()));
        }
    }

    deliveries
}

/// Apply simple relay doctrine to fresh inbox entries.
///
/// - Direct packets addressed to this unit stay in the inbox for consumers.
/// - Direct packets addressed to someone else are moved to the outbox unchanged.
/// - Broadcast packets stay in the inbox and are also copied to the outbox.
///
/// Because this runs after delivery, relayed packets transmit on the next comms
/// pass rather than in the same tick.
fn relay_packets(
    mut units: Query<(Entity, &mut Inbox, &mut Outbox), (With<Soldier>, With<Alive>)>,
) {
    for (entity, mut inbox, mut outbox) in &mut units {
        relay_inbox_for_entity(entity, &mut inbox, &mut outbox);
    }
}

fn relay_inbox_for_entity(entity: Entity, inbox: &mut Inbox, outbox: &mut Outbox) {
    let mut retained = Vec::with_capacity(inbox.packets.len());

    for mut entry in inbox.packets.drain(..) {
        if !entry.fresh {
            retained.push(entry);
            continue;
        }

        match entry.packet.address {
            Address::Direct(target) if target != entity => {
                outbox.packets.push(entry.packet);
            }
            Address::Direct(_) => {
                entry.fresh = false;
                retained.push(entry);
            }
            Address::Broadcast => {
                outbox.packets.push(entry.packet.clone());
                entry.fresh = false;
                retained.push(entry);
            }
        }
    }

    inbox.packets = retained;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::units::{Dead, Rank, Role};
    use crate::gameplay::comms::{CommsKind, CommsLink, CommsLinks};
    use bevy::ecs::system::RunSystemOnce;
    use std::collections::{HashMap, HashSet};

    fn packet(origin: Entity, address: Address, id: u64, subject: Entity) -> InfoPacket {
        InfoPacket {
            id: PacketId(id),
            origin,
            address,
            created_tick: 7,
            payload: PacketPayload::ContactReport(ContactClaim {
                subject,
                position_m: Vec2::new(1.0, 2.0),
                observed_tick: 6,
                life_status: ReportedLifeStatus::Alive,
                contact_type: ContactType::Hostile,
            }),
        }
    }

    fn spawn_packet_soldier(world: &mut World, alive: bool) -> Entity {
        let mut entity = world.spawn((
            Soldier {
                rank: Rank::Private,
                role: Role::Rifleman,
            },
            Inbox::default(),
            Outbox::default(),
            SeenPackets::default(),
            CommsLinks::default(),
        ));

        if alive {
            entity.insert(Alive);
        } else {
            entity.insert(Dead);
        }

        entity.id()
    }

    #[test]
    fn outbox_send_allocates_packet_and_marks_sender_seen() {
        let mut world = World::new();
        let origin = world.spawn_empty().id();
        let target = world.spawn_empty().id();
        let mut ids = PacketIdAllocator::default();
        let mut seen = SeenPackets::default();
        let mut outbox = Outbox::default();

        let id = outbox.send(
            &mut seen,
            &mut ids,
            origin,
            Address::Direct(target),
            3,
            PacketPayload::ContactReport(ContactClaim {
                subject: target,
                position_m: Vec2::ZERO,
                observed_tick: 2,
                life_status: ReportedLifeStatus::Alive,
                contact_type: ContactType::Hostile,
            }),
        );

        assert_eq!(id, PacketId(0));
        assert!(seen.ids.contains(&id));
        assert_eq!(outbox.packets.len(), 1);
        assert_eq!(outbox.packets[0].origin, origin);
        assert_eq!(outbox.packets[0].address, Address::Direct(target));
    }

    #[test]
    fn packet_id_allocator_default_resets_sequence() {
        let mut ids = PacketIdAllocator::default();
        assert_eq!(ids.allocate(), PacketId(0));
        assert_eq!(ids.allocate(), PacketId(1));

        ids = PacketIdAllocator::default();
        assert_eq!(ids.allocate(), PacketId(0));
    }

    #[test]
    fn prune_inbox_drops_stale_entries_and_keeps_fresh_entries() {
        let mut world = World::new();
        let origin = world.spawn_empty().id();
        let target = world.spawn_empty().id();
        let stale = InfoPacket {
            created_tick: 10,
            ..packet(origin, Address::Direct(target), 1, target)
        };
        let fresh = InfoPacket {
            created_tick: 100,
            ..packet(origin, Address::Direct(target), 2, target)
        };
        let mut inbox = Inbox {
            packets: vec![
                InboxEntry {
                    packet: stale,
                    fresh: false,
                },
                InboxEntry {
                    packet: fresh.clone(),
                    fresh: true,
                },
            ],
        };

        prune_inbox(&mut inbox, 10 + INBOX_TTL_TICKS + 1);

        assert_eq!(inbox.packets.len(), 1);
        assert_eq!(inbox.packets[0].packet, fresh);
        assert!(inbox.packets[0].fresh);
    }

    #[test]
    fn prune_inbox_does_not_touch_seen_packets() {
        let mut world = World::new();
        let origin = world.spawn_empty().id();
        let target = world.spawn_empty().id();
        let mut inbox = Inbox {
            packets: vec![InboxEntry {
                packet: InfoPacket {
                    created_tick: 1,
                    ..packet(origin, Address::Direct(target), 1, target)
                },
                fresh: false,
            }],
        };
        let mut seen = SeenPackets::default();
        seen.ids.insert(PacketId(1));

        prune_inbox(&mut inbox, 1 + INBOX_TTL_TICKS + 1);

        assert!(inbox.packets.is_empty());
        assert!(seen.ids.contains(&PacketId(1)));
    }

    #[test]
    fn direct_delivery_plans_to_linked_target() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let contact = world.spawn_empty().id();
        let pkt = packet(a, Address::Direct(b), 1, contact);

        let deliveries = plan_deliveries(
            &[(a, pkt.clone())],
            |sender| if sender == a { vec![b] } else { vec![] },
            |_, _| false,
        );

        assert_eq!(deliveries, vec![(b, pkt)]);
    }

    #[test]
    fn addressed_packets_are_physically_broadcast_to_all_neighbors() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let c = world.spawn_empty().id();
        let contact = world.spawn_empty().id();
        let pkt = packet(a, Address::Direct(b), 1, contact);

        let deliveries = plan_deliveries(
            &[(a, pkt.clone())],
            |sender| if sender == a { vec![b, c] } else { vec![] },
            |_, _| false,
        );

        assert_eq!(deliveries, vec![(b, pkt.clone()), (c, pkt)]);
    }

    #[test]
    fn already_seen_receivers_are_skipped() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let c = world.spawn_empty().id();
        let contact = world.spawn_empty().id();
        let pkt = packet(a, Address::Broadcast, 1, contact);
        let mut seen: HashSet<(Entity, PacketId)> = [(b, PacketId(1))].into();

        let deliveries = plan_deliveries(
            &[(a, pkt.clone())],
            |sender| if sender == a { vec![b, c] } else { vec![] },
            |receiver, packet_id| seen.contains(&(receiver, packet_id)),
        );

        assert_eq!(deliveries, vec![(c, pkt)]);
        seen.insert((c, PacketId(1)));
        assert!(seen.contains(&(b, PacketId(1))));
    }

    #[test]
    fn helper_handles_multiple_sender_topologies() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let c = world.spawn_empty().id();
        let d = world.spawn_empty().id();
        let contact = world.spawn_empty().id();
        let pkt_a = packet(a, Address::Broadcast, 1, contact);
        let pkt_c = packet(c, Address::Direct(d), 2, contact);
        let links: HashMap<Entity, Vec<Entity>> = [(a, vec![b]), (c, vec![b, d])].into();

        let deliveries = plan_deliveries(
            &[(a, pkt_a.clone()), (c, pkt_c.clone())],
            |sender| links.get(&sender).cloned().unwrap_or_default(),
            |_, _| false,
        );

        assert_eq!(deliveries, vec![(b, pkt_a), (b, pkt_c.clone()), (d, pkt_c)]);
    }

    #[test]
    fn relay_moves_fresh_direct_packet_not_addressed_to_me() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let leader = world.spawn_empty().id();
        let contact = world.spawn_empty().id();
        let pkt = packet(a, Address::Direct(leader), 1, contact);
        let mut inbox = Inbox {
            packets: vec![InboxEntry {
                packet: pkt.clone(),
                fresh: true,
            }],
        };
        let mut outbox = Outbox::default();

        relay_inbox_for_entity(b, &mut inbox, &mut outbox);

        assert!(inbox.packets.is_empty());
        assert_eq!(outbox.packets, vec![pkt]);
    }

    #[test]
    fn relay_keeps_fresh_direct_packet_addressed_to_me() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let me = world.spawn_empty().id();
        let contact = world.spawn_empty().id();
        let pkt = packet(a, Address::Direct(me), 1, contact);
        let mut inbox = Inbox {
            packets: vec![InboxEntry {
                packet: pkt.clone(),
                fresh: true,
            }],
        };
        let mut outbox = Outbox::default();

        relay_inbox_for_entity(me, &mut inbox, &mut outbox);

        assert!(outbox.packets.is_empty());
        assert_eq!(inbox.packets.len(), 1);
        assert_eq!(inbox.packets[0].packet, pkt);
        assert!(!inbox.packets[0].fresh);
    }

    #[test]
    fn relay_keeps_and_rebroadcasts_fresh_broadcast_packet_once() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let me = world.spawn_empty().id();
        let contact = world.spawn_empty().id();
        let pkt = packet(a, Address::Broadcast, 1, contact);
        let mut inbox = Inbox {
            packets: vec![InboxEntry {
                packet: pkt.clone(),
                fresh: true,
            }],
        };
        let mut outbox = Outbox::default();

        relay_inbox_for_entity(me, &mut inbox, &mut outbox);
        relay_inbox_for_entity(me, &mut inbox, &mut outbox);

        assert_eq!(inbox.packets.len(), 1);
        assert_eq!(inbox.packets[0].packet, pkt.clone());
        assert!(!inbox.packets[0].fresh);
        assert_eq!(outbox.packets, vec![pkt]);
    }

    #[test]
    fn relayed_packet_reaches_two_hop_target_on_second_delivery_pass() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let c = world.spawn_empty().id();
        let contact = world.spawn_empty().id();
        let pkt = packet(a, Address::Direct(c), 1, contact);
        let links: HashMap<Entity, Vec<Entity>> = [(a, vec![b]), (b, vec![a, c])].into();
        let mut seen: HashSet<(Entity, PacketId)> = [(a, pkt.id)].into();
        let mut inbox_b = Inbox::default();
        let mut inbox_c = Inbox::default();
        let mut outbox_b = Outbox::default();

        let first_hop = plan_deliveries(
            &[(a, pkt.clone())],
            |sender| links.get(&sender).cloned().unwrap_or_default(),
            |receiver, packet_id| seen.contains(&(receiver, packet_id)),
        );
        for (receiver, packet) in first_hop {
            seen.insert((receiver, packet.id));
            if receiver == b {
                inbox_b.packets.push(InboxEntry {
                    packet,
                    fresh: true,
                });
            }
        }

        relay_inbox_for_entity(b, &mut inbox_b, &mut outbox_b);

        assert!(inbox_b.packets.is_empty());
        assert_eq!(outbox_b.packets, vec![pkt.clone()]);
        assert!(
            !seen.contains(&(c, pkt.id)),
            "c has not heard the packet on tick 1"
        );

        let second_hop_sends: Vec<_> = outbox_b
            .packets
            .drain(..)
            .map(|packet| (b, packet))
            .collect();
        let second_hop = plan_deliveries(
            &second_hop_sends,
            |sender| links.get(&sender).cloned().unwrap_or_default(),
            |receiver, packet_id| seen.contains(&(receiver, packet_id)),
        );
        for (receiver, packet) in second_hop {
            seen.insert((receiver, packet.id));
            if receiver == c {
                inbox_c.packets.push(InboxEntry {
                    packet,
                    fresh: true,
                });
            }
        }

        assert_eq!(inbox_c.packets.len(), 1);
        assert_eq!(inbox_c.packets[0].packet, pkt);
    }

    #[test]
    fn leader_does_not_receive_duplicate_from_relaying_squadmates() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let c = world.spawn_empty().id();
        let leader = world.spawn_empty().id();
        let contact = world.spawn_empty().id();
        let pkt = packet(a, Address::Direct(leader), 1, contact);
        let links: HashMap<Entity, Vec<Entity>> = [
            (a, vec![b, c, leader]),
            (b, vec![a, c, leader]),
            (c, vec![a, b, leader]),
        ]
        .into();
        let mut seen: HashSet<(Entity, PacketId)> = [(a, pkt.id)].into();
        let mut inbox_b = Inbox::default();
        let mut inbox_c = Inbox::default();
        let mut inbox_leader = Inbox::default();
        let mut outbox_b = Outbox::default();
        let mut outbox_c = Outbox::default();

        let first_hop = plan_deliveries(
            &[(a, pkt.clone())],
            |sender| links.get(&sender).cloned().unwrap_or_default(),
            |receiver, packet_id| seen.contains(&(receiver, packet_id)),
        );
        for (receiver, packet) in first_hop {
            seen.insert((receiver, packet.id));
            let inbox = if receiver == b {
                &mut inbox_b
            } else if receiver == c {
                &mut inbox_c
            } else if receiver == leader {
                &mut inbox_leader
            } else {
                continue;
            };
            inbox.packets.push(InboxEntry {
                packet,
                fresh: true,
            });
        }

        relay_inbox_for_entity(b, &mut inbox_b, &mut outbox_b);
        relay_inbox_for_entity(c, &mut inbox_c, &mut outbox_c);
        relay_inbox_for_entity(leader, &mut inbox_leader, &mut Outbox::default());

        assert_eq!(inbox_leader.packets.len(), 1);
        assert_eq!(outbox_b.packets, vec![pkt.clone()]);
        assert_eq!(outbox_c.packets, vec![pkt.clone()]);

        let relays: Vec<_> = outbox_b
            .packets
            .drain(..)
            .map(|packet| (b, packet))
            .chain(outbox_c.packets.drain(..).map(|packet| (c, packet)))
            .collect();
        let second_hop = plan_deliveries(
            &relays,
            |sender| links.get(&sender).cloned().unwrap_or_default(),
            |receiver, packet_id| seen.contains(&(receiver, packet_id)),
        );

        assert!(
            second_hop.iter().all(|(receiver, _)| *receiver != leader),
            "leader already saw the packet and should reject relayed duplicates"
        );
    }

    #[test]
    fn broadcast_flood_halts_without_duplicate_inbox_entries() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let c = world.spawn_empty().id();
        let contact = world.spawn_empty().id();
        let pkt = packet(a, Address::Broadcast, 1, contact);
        let links: HashMap<Entity, Vec<Entity>> =
            [(a, vec![b, c]), (b, vec![a, c]), (c, vec![a, b])].into();
        let mut seen: HashSet<(Entity, PacketId)> = [(a, pkt.id)].into();
        let mut inboxes: HashMap<Entity, Inbox> = [
            (a, Inbox::default()),
            (b, Inbox::default()),
            (c, Inbox::default()),
        ]
        .into();
        let mut outboxes: HashMap<Entity, Outbox> = [
            (
                a,
                Outbox {
                    packets: vec![pkt.clone()],
                },
            ),
            (b, Outbox::default()),
            (c, Outbox::default()),
        ]
        .into();

        for _ in 0..5 {
            let mut sends = Vec::new();
            for (sender, outbox) in &mut outboxes {
                sends.extend(outbox.packets.drain(..).map(|packet| (*sender, packet)));
            }

            let deliveries = plan_deliveries(
                &sends,
                |sender| links.get(&sender).cloned().unwrap_or_default(),
                |receiver, packet_id| seen.contains(&(receiver, packet_id)),
            );

            for (receiver, packet) in deliveries {
                if !seen.insert((receiver, packet.id)) {
                    continue;
                }
                inboxes
                    .get_mut(&receiver)
                    .unwrap()
                    .packets
                    .push(InboxEntry {
                        packet,
                        fresh: true,
                    });
            }

            for entity in [a, b, c] {
                let inbox = inboxes.get_mut(&entity).unwrap();
                let outbox = outboxes.get_mut(&entity).unwrap();
                relay_inbox_for_entity(entity, inbox, outbox);
            }
        }

        assert!(inboxes.get(&a).unwrap().packets.is_empty());
        assert_eq!(inboxes.get(&b).unwrap().packets.len(), 1);
        assert_eq!(inboxes.get(&c).unwrap().packets.len(), 1);
        assert_eq!(inboxes.get(&b).unwrap().packets[0].packet, pkt);
        assert_eq!(inboxes.get(&c).unwrap().packets[0].packet, pkt);
        assert!(outboxes.values().all(|outbox| outbox.packets.is_empty()));
    }

    #[test]
    fn sender_never_self_receives_broadcast_relay() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let contact = world.spawn_empty().id();
        let pkt = packet(a, Address::Broadcast, 1, contact);
        let links: HashMap<Entity, Vec<Entity>> = [(a, vec![b]), (b, vec![a])].into();
        let mut seen: HashSet<(Entity, PacketId)> = [(a, pkt.id)].into();
        let mut inbox_b = Inbox::default();
        let mut outbox_b = Outbox::default();

        let first_hop = plan_deliveries(
            &[(a, pkt.clone())],
            |sender| links.get(&sender).cloned().unwrap_or_default(),
            |receiver, packet_id| seen.contains(&(receiver, packet_id)),
        );
        for (receiver, packet) in first_hop {
            seen.insert((receiver, packet.id));
            if receiver == b {
                inbox_b.packets.push(InboxEntry {
                    packet,
                    fresh: true,
                });
            }
        }

        relay_inbox_for_entity(b, &mut inbox_b, &mut outbox_b);
        let second_hop = plan_deliveries(
            &[(b, outbox_b.packets.remove(0))],
            |sender| links.get(&sender).cloned().unwrap_or_default(),
            |receiver, packet_id| seen.contains(&(receiver, packet_id)),
        );

        assert!(second_hop.iter().all(|(receiver, _)| *receiver != a));
    }

    #[test]
    fn dead_units_do_not_receive_delivered_packets() {
        let mut world = World::new();
        world.init_resource::<CommsGraph>();
        let a = spawn_packet_soldier(&mut world, true);
        let b = spawn_packet_soldier(&mut world, false);
        let contact = world.spawn_empty().id();
        let pkt = packet(a, Address::Direct(b), 1, contact);

        world.entity_mut(a).insert(CommsLinks {
            links: vec![CommsLink {
                target: b,
                kind: CommsKind::Voice,
            }],
        });
        world.get_mut::<SeenPackets>(a).unwrap().ids.insert(pkt.id);
        world.get_mut::<Outbox>(a).unwrap().packets.push(pkt);

        world.run_system_once(update_comms_graph).unwrap();
        world.run_system_once(deliver_packets).unwrap();

        assert!(world.get::<Inbox>(b).unwrap().packets.is_empty());
        assert!(world.get::<SeenPackets>(b).unwrap().ids.is_empty());
    }
}
