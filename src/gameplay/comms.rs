#![doc = include_str!("../../docs/gameplay/comms.md")]

use crate::ai::perception::{ContactKind, ContactType, PerceptionMemory};
use crate::gameplay::simulation::{SimulationClock, SimulationSet};
use crate::units::{Allegiance, Side, Soldier};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

pub struct CommsPlugin;

impl Plugin for CommsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CommsGraph>().add_systems(
            FixedUpdate,
            (update_voice_comms, update_comms_graph)
                .chain()
                .in_set(SimulationSet::Comms),
        );
    }
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct VoiceComms;

#[derive(Component, Debug, Default)]
pub struct CommsLinks {
    pub links: Vec<CommsLink>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct CommsLink {
    pub target: Entity,
    pub kind: CommsKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommsKind {
    Voice,
}

#[derive(Resource, Debug, Default)]
pub struct CommsGraph {
    adjacency: HashMap<Entity, Vec<CommsLink>>,
}

impl CommsGraph {
    pub fn reachable_from(
        &self,
        root: Entity,
        side: Side,
        mut side_of: impl FnMut(Entity) -> Option<Side>,
    ) -> HashSet<Entity> {
        let mut reachable = HashSet::new();
        let mut frontier = vec![root];

        while let Some(entity) = frontier.pop() {
            if !reachable.insert(entity) {
                continue;
            }

            if side_of(entity) != Some(side) {
                continue;
            }

            let Some(links) = self.adjacency.get(&entity) else {
                continue;
            };

            for link in links {
                if !reachable.contains(&link.target) {
                    frontier.push(link.target);
                }
            }
        }

        reachable
    }

    pub fn can_reach(
        &self,
        root: Entity,
        target: Entity,
        side: Side,
        side_of: impl FnMut(Entity) -> Option<Side>,
    ) -> bool {
        self.reachable_from(root, side, side_of).contains(&target)
    }

    pub fn links_from(&self, entity: Entity) -> Option<&[CommsLink]> {
        self.adjacency.get(&entity).map(Vec::as_slice)
    }
}

#[derive(Debug, Clone, Copy)]
struct CommsSnapshot<'a> {
    entity: Entity,
    side: crate::units::Side,
    voice: Option<VoiceComms>,
    memory: &'a PerceptionMemory,
}

fn update_voice_comms(
    clock: Res<SimulationClock>,
    snapshots: Query<(Entity, &Allegiance, Option<&VoiceComms>, &PerceptionMemory), With<Soldier>>,
    mut links_query: Query<&mut CommsLinks, With<Soldier>>,
) {
    let snapshots: Vec<CommsSnapshot> = snapshots
        .iter()
        .map(|(entity, allegiance, voice, memory)| CommsSnapshot {
            entity,
            side: allegiance.side,
            voice: voice.copied(),
            memory,
        })
        .collect();

    for mut links in &mut links_query {
        links.links.clear();
    }

    for source in &snapshots {
        if source.voice.is_none() {
            continue;
        };

        let Ok(mut links) = links_query.get_mut(source.entity) else {
            continue;
        };

        for contact in &source.memory.contacts {
            if contact.last_seen_tick != clock.tick
                || contact.kind != ContactKind::Auditory
                || contact.contact_type != ContactType::Friendly
            {
                continue;
            }

            let Some(target) = snapshots
                .iter()
                .find(|target| target.entity == contact.target)
            else {
                continue;
            };

            if target.side != source.side || target.voice.is_none() {
                continue;
            }

            links.links.push(CommsLink {
                target: target.entity,
                kind: CommsKind::Voice,
            });
        }
    }
}

fn update_comms_graph(
    links_query: Query<(Entity, &CommsLinks), With<Soldier>>,
    mut graph: ResMut<CommsGraph>,
) {
    graph.adjacency.clear();

    for (entity, links) in &links_query {
        graph.adjacency.insert(entity, links.links.clone());
    }
}
