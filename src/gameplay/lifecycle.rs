#![doc = include_str!("../../docs/gameplay/lifecycle.md")]

use crate::GameState;
use crate::actors::units::{Alive, Dead, Soldier};
use crate::ai::perception::{AuditorySensor, EyeHeight, PerceptionMemory, VisualSensor};
use crate::gameplay::combat::{CombatOrder, CombatState};
use crate::gameplay::comms::{CommsLinks, VoiceComms};
use crate::gameplay::orders::{CombatOrderSource, MovementOrderSource};
use crate::gameplay::packets::{Inbox, Outbox, SeenPackets};
use crate::gameplay::simulation::MovementOrder;
use crate::player::knowledge::ReportCadence;
use bevy::prelude::*;

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitDied {
    pub entity: Entity,
    pub tick: u64,
    pub cause: UnitDeathCause,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitDeathCause {
    Combat { attacker: Entity },
    Debug,
}

/// Marks entities whose lifetime is scoped to the active mission.
#[derive(Component)]
pub struct MissionScoped;

pub struct UnitLifecyclePlugin;

impl Plugin for UnitLifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UnitDied>().add_systems(
            Update,
            warn_invalid_life_status.run_if(in_state(GameState::MissionScreen)),
        );
    }
}

/// Transition a unit from alive actor to dead ground-truth entity.
///
/// The entity is intentionally not despawned: player knowledge may remain stale,
/// and living units may later observe the body. Death strips active capabilities
/// so the entity can no longer move, perceive, communicate, or remember/report.
pub fn kill_unit(commands: &mut Commands, entity: Entity, tick: u64, cause: UnitDeathCause) {
    // Check life state when deferred commands are applied, not when death is
    // requested. Multiple lethal hits in one system can otherwise all observe
    // Alive before the first removal is flushed and emit duplicate deaths.
    commands.queue(move |world: &mut World| {
        let Ok(mut unit) = world.get_entity_mut(entity) else {
            return;
        };
        if !unit.contains::<Alive>() {
            return;
        }

        unit.remove::<Alive>()
            .insert(Dead)
            .remove::<MovementOrder>()
            .remove::<MovementOrderSource>()
            .remove::<CombatOrder>()
            .remove::<CombatOrderSource>()
            .remove::<CombatState>()
            .remove::<VisualSensor>()
            .remove::<AuditorySensor>()
            .remove::<EyeHeight>()
            .remove::<VoiceComms>()
            .remove::<CommsLinks>()
            .remove::<PerceptionMemory>()
            .remove::<Inbox>()
            .remove::<Outbox>()
            .remove::<SeenPackets>()
            .remove::<ReportCadence>();

        world.write_message(UnitDied {
            entity,
            tick,
            cause,
        });
    });
}

fn warn_invalid_life_status(
    both: Query<Entity, (With<Soldier>, With<Alive>, With<Dead>)>,
    neither: Query<Entity, (With<Soldier>, Without<Alive>, Without<Dead>)>,
) {
    for entity in &both {
        error!("Soldier has both Alive and Dead markers: {entity:?}");
    }

    for entity in &neither {
        error!("Soldier has neither Alive nor Dead marker: {entity:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::world::CommandQueue;

    fn apply_kills(
        world: &mut World,
        requests: impl IntoIterator<Item = (Entity, u64, UnitDeathCause)>,
    ) {
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            for (entity, tick, cause) in requests {
                kill_unit(&mut commands, entity, tick, cause);
            }
        }
        queue.apply(world);
    }

    fn drain_deaths(world: &mut World) -> Vec<UnitDied> {
        world.resource_mut::<Messages<UnitDied>>().drain().collect()
    }

    #[test]
    fn death_strips_capabilities_and_emits_cause() {
        let mut world = World::new();
        world.insert_resource(Messages::<UnitDied>::default());
        let attacker = world.spawn_empty().id();
        let entity = world
            .spawn((
                Alive,
                MovementOrder::Hold,
                MovementOrderSource::player(),
                CombatOrder::HoldFire,
                CombatOrderSource::player(),
                CombatState::default(),
                VisualSensor::default(),
                AuditorySensor::default(),
                EyeHeight::default(),
                VoiceComms,
                CommsLinks::default(),
                PerceptionMemory::default(),
                Inbox::default(),
                Outbox::default(),
                SeenPackets::default(),
            ))
            .insert(ReportCadence::default())
            .id();

        apply_kills(
            &mut world,
            [(entity, 42, UnitDeathCause::Combat { attacker })],
        );

        assert!(!world.entity(entity).contains::<Alive>());
        assert!(world.entity(entity).contains::<Dead>());
        assert!(!world.entity(entity).contains::<MovementOrder>());
        assert!(!world.entity(entity).contains::<MovementOrderSource>());
        assert!(!world.entity(entity).contains::<CombatOrder>());
        assert!(!world.entity(entity).contains::<CombatOrderSource>());
        assert!(!world.entity(entity).contains::<CombatState>());
        assert!(!world.entity(entity).contains::<VisualSensor>());
        assert!(!world.entity(entity).contains::<AuditorySensor>());
        assert!(!world.entity(entity).contains::<EyeHeight>());
        assert!(!world.entity(entity).contains::<VoiceComms>());
        assert!(!world.entity(entity).contains::<CommsLinks>());
        assert!(!world.entity(entity).contains::<PerceptionMemory>());
        assert!(!world.entity(entity).contains::<Inbox>());
        assert!(!world.entity(entity).contains::<Outbox>());
        assert!(!world.entity(entity).contains::<SeenPackets>());
        assert!(!world.entity(entity).contains::<ReportCadence>());
        assert_eq!(
            drain_deaths(&mut world),
            vec![UnitDied {
                entity,
                tick: 42,
                cause: UnitDeathCause::Combat { attacker },
            }]
        );
    }

    #[test]
    fn repeated_death_requests_before_flush_emit_once() {
        let mut world = World::new();
        world.insert_resource(Messages::<UnitDied>::default());
        let entity = world.spawn(Alive).id();

        apply_kills(
            &mut world,
            [
                (entity, 7, UnitDeathCause::Debug),
                (entity, 8, UnitDeathCause::Debug),
            ],
        );

        assert_eq!(
            drain_deaths(&mut world),
            vec![UnitDied {
                entity,
                tick: 7,
                cause: UnitDeathCause::Debug,
            }]
        );
    }

    #[test]
    fn already_dead_or_missing_entities_do_not_emit() {
        let mut world = World::new();
        world.insert_resource(Messages::<UnitDied>::default());
        let dead = world.spawn(Dead).id();
        let missing = world.spawn_empty().id();
        world.despawn(missing);

        apply_kills(
            &mut world,
            [
                (dead, 3, UnitDeathCause::Debug),
                (missing, 3, UnitDeathCause::Debug),
            ],
        );

        assert!(drain_deaths(&mut world).is_empty());
    }
}
