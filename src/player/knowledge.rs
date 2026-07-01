#![doc = include_str!("../../docs/player/knowledge.md")]

use crate::GameState;
use crate::actors::units::{Alive, Allegiance, Side, Soldier};
use crate::ai::perception::PerceptionMemory;
use crate::gameplay::comms::CommsGraph;
use crate::gameplay::components::BattlefieldPosition;
use crate::gameplay::simulation::{SimulationClock, SimulationSet};
use crate::intel::ReportedLifeStatus;
use bevy::prelude::*;

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
    pub reported_life_status: ReportedLifeStatus,
    pub last_known_position_m: Vec2,
    /// Tick when this unit/contact was physically observed by the reporting unit.
    pub last_observed_tick: u64,
    /// Tick when the player received or refreshed the report through comms.
    pub last_reported_tick: u64,
}

fn update_player_tactical_knowledge(
    clock: Res<SimulationClock>,
    controlled: Query<Entity, (With<PlayerControlledUnit>, With<Alive>)>,
    graph: Res<CommsGraph>,
    units: Query<
        (
            Entity,
            &BattlefieldPosition,
            &Allegiance,
            Option<&PerceptionMemory>,
        ),
        (With<Soldier>, With<Alive>),
    >,
    target_units: Query<(&BattlefieldPosition, &Allegiance), With<Soldier>>,
    mut knowledge: ResMut<PlayerTacticalKnowledge>,
) {
    let Ok(controlled_entity) = controlled.single() else {
        return;
    };

    let Ok((_, _, controlled_allegiance, _)) = units.get(controlled_entity) else {
        return;
    };
    let player_side = controlled_allegiance.side;

    let reachable = graph.reachable_from(controlled_entity, player_side, |entity| {
        units
            .get(entity)
            .ok()
            .map(|(_, _, allegiance, _)| allegiance.side)
    });

    // First fold in sensor contacts from reachable friendly units. These reports may include
    // friendlies outside the comms graph, but they should not override direct position reports
    // for units that are already reachable through comms.
    for entity in &reachable {
        let Ok((_, _, _, memory)) = units.get(*entity) else {
            continue;
        };

        let Some(memory) = memory else {
            continue;
        };

        for contact in &memory.contacts {
            if contact.last_seen_tick != clock.tick {
                continue;
            }

            let Ok((target_position, target_allegiance)) = target_units.get(contact.target) else {
                continue;
            };

            knowledge.upsert_report(KnownUnit {
                entity: contact.target,
                side: target_allegiance.side,
                reported_life_status: contact.observed_life_status,
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

    // Then write direct reports for units in the comms graph. These are authoritative for
    // friendlies the player can currently communicate with.
    for entity in &reachable {
        let Ok((_, position, allegiance, _)) = units.get(*entity) else {
            continue;
        };

        knowledge.upsert_report(KnownUnit {
            entity: *entity,
            side: allegiance.side,
            reported_life_status: ReportedLifeStatus::Alive,
            last_known_position_m: position.0,
            last_observed_tick: clock.tick,
            last_reported_tick: clock.tick,
        });
    }
}
