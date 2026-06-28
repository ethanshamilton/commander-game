use crate::GameState;
use crate::ai::perception::PerceptionMemory;
use crate::gameplay::comms::CommsLinks;
use crate::gameplay::components::BattlefieldPosition;
use crate::gameplay::simulation::{SimulationClock, SimulationSet};
use crate::units::{Allegiance, Side, Soldier};
use bevy::prelude::*;
use std::collections::HashSet;

pub struct PlayerKnowledgePlugin;

impl Plugin for PlayerKnowledgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerTacticalKnowledge>().add_systems(
            FixedUpdate,
            update_player_tactical_knowledge
                .in_set(SimulationSet::Reports)
                .run_if(in_state(GameState::MissionScreen)),
        );
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct PlayerControlledUnit;

#[derive(Resource, Debug, Default)]
pub struct PlayerTacticalKnowledge {
    pub units: Vec<KnownUnit>,
}

impl PlayerTacticalKnowledge {
    pub fn get(&self, entity: Entity) -> Option<&KnownUnit> {
        self.units.iter().find(|unit| unit.entity == entity)
    }

    pub fn is_current(&self, entity: Entity, tick: u64) -> bool {
        self.get(entity)
            .is_some_and(|unit| unit.last_reported_tick == tick)
    }

    fn upsert_report(&mut self, report: KnownUnit) {
        if let Some(existing) = self
            .units
            .iter_mut()
            .find(|existing| existing.entity == report.entity)
        {
            if report.last_observed_tick >= existing.last_observed_tick
                || report.last_reported_tick >= existing.last_reported_tick
            {
                *existing = report;
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
    pub last_known_position_m: Vec2,
    /// Tick when this unit/contact was physically observed by the reporting unit.
    pub last_observed_tick: u64,
    /// Tick when the player received or refreshed the report through comms.
    pub last_reported_tick: u64,
}

fn update_player_tactical_knowledge(
    clock: Res<SimulationClock>,
    controlled: Query<Entity, With<PlayerControlledUnit>>,
    units: Query<
        (
            Entity,
            &BattlefieldPosition,
            &Allegiance,
            &CommsLinks,
            Option<&PerceptionMemory>,
        ),
        With<Soldier>,
    >,
    target_units: Query<(&BattlefieldPosition, &Allegiance), With<Soldier>>,
    mut knowledge: ResMut<PlayerTacticalKnowledge>,
) {
    let Ok(controlled_entity) = controlled.single() else {
        return;
    };

    let Ok((_, _, controlled_allegiance, _, _)) = units.get(controlled_entity) else {
        return;
    };
    let player_side = controlled_allegiance.side;

    let reachable = reachable_friendly_units(controlled_entity, player_side, |entity| {
        let Ok((_, _, allegiance, comms, _)) = units.get(entity) else {
            return None;
        };
        Some((
            allegiance.side,
            comms.links.iter().map(|link| link.target).collect(),
        ))
    });

    for entity in &reachable {
        let Ok((_, position, allegiance, _, memory)) = units.get(*entity) else {
            continue;
        };

        knowledge.upsert_report(KnownUnit {
            entity: *entity,
            side: allegiance.side,
            last_known_position_m: position.0,
            last_observed_tick: clock.tick,
            last_reported_tick: clock.tick,
        });

        let Some(memory) = memory else {
            continue;
        };

        for contact in &memory.contacts {
            let Ok((target_position, target_allegiance)) = target_units.get(contact.target) else {
                continue;
            };

            if target_allegiance.side == player_side {
                continue;
            }

            knowledge.upsert_report(KnownUnit {
                entity: contact.target,
                side: target_allegiance.side,
                last_known_position_m: if contact.last_seen_tick == clock.tick {
                    target_position.0
                } else {
                    contact.last_seen_position_m
                },
                last_observed_tick: contact.last_seen_tick,
                last_reported_tick: clock.tick,
            });
        }
    }
}

pub fn reachable_friendly_units(
    root: Entity,
    side: Side,
    mut unit_links: impl FnMut(Entity) -> Option<(Side, Vec<Entity>)>,
) -> HashSet<Entity> {
    let mut reachable = HashSet::new();
    let mut frontier = vec![root];

    while let Some(entity) = frontier.pop() {
        if !reachable.insert(entity) {
            continue;
        }

        let Some((unit_side, links)) = unit_links(entity) else {
            continue;
        };

        if unit_side != side {
            continue;
        }

        for target in links {
            if !reachable.contains(&target) {
                frontier.push(target);
            }
        }
    }

    reachable
}
