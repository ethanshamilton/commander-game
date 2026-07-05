#![doc = include_str!("../../docs/gameplay/orders.md")]

use bevy::prelude::*;
use std::marker::PhantomData;

/// Who issued an order. Provenance decides arbitration: HTN planning yields to
/// Player orders; Doctrine marks default postures that never suppress planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSource {
    /// Direct player directive. Preempts autonomous planning.
    Player,
    /// Issued by the unit's own HTN executor.
    Htn,
    /// Default posture (e.g. spawn-time HoldFire, combat-resolution decay).
    /// Never treated as a directive.
    Doctrine,
    // Future: Superior(Entity) — delegated order received via comms.
}

/// Provenance of an order component of type `O`.
///
/// INVARIANT: `OrderProvenance::<O>` is present if and only if `O` is present
/// on the same entity. Every site that inserts/removes/overwrites the order
/// component must do the same to its provenance in the same command batch.
///
/// The `PhantomData<O>` marker exists purely so `OrderProvenance<UnitOrder>`
/// and `OrderProvenance<CombatOrder>` are distinct component types (ECS keys
/// components by concrete type) while sharing all logic.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderProvenance<O: Component> {
    pub source: OrderSource,
    _marker: PhantomData<O>,
}

impl<O: Component> OrderProvenance<O> {
    pub const fn new(source: OrderSource) -> Self {
        Self {
            source,
            _marker: PhantomData,
        }
    }

    pub const fn player() -> Self {
        Self::new(OrderSource::Player)
    }

    pub const fn htn() -> Self {
        Self::new(OrderSource::Htn)
    }

    pub const fn doctrine() -> Self {
        Self::new(OrderSource::Doctrine)
    }
}

/// True if the order (if present) came from the player and must preempt HTN planning.
pub fn is_player_sourced<O: Component>(src: Option<&OrderProvenance<O>>) -> bool {
    src.is_some_and(|s| s.source == OrderSource::Player)
}

/// Remove an HTN-sourced order and its provenance. No-op for other sources.
pub fn clear_if_htn<O: Component>(
    commands: &mut Commands,
    entity: Entity,
    src: Option<&OrderProvenance<O>>,
) {
    if src.is_some_and(|s| s.source == OrderSource::Htn) {
        commands.entity(entity).remove::<(O, OrderProvenance<O>)>();
    }
}

pub type UnitOrderSource = OrderProvenance<crate::gameplay::simulation::UnitOrder>;
pub type CombatOrderSource = OrderProvenance<crate::gameplay::combat::CombatOrder>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::combat::CombatOrder;
    use bevy::ecs::world::CommandQueue;

    #[test]
    fn clear_if_htn_removes_htn_sourced_order_only() {
        let mut world = World::new();
        let htn_entity = world
            .spawn((CombatOrder::HoldFire, CombatOrderSource::htn()))
            .id();
        let player_entity = world
            .spawn((CombatOrder::HoldFire, CombatOrderSource::player()))
            .id();

        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, &world);
            clear_if_htn::<CombatOrder>(
                &mut commands,
                htn_entity,
                world.get::<CombatOrderSource>(htn_entity),
            );
            clear_if_htn::<CombatOrder>(
                &mut commands,
                player_entity,
                world.get::<CombatOrderSource>(player_entity),
            );
        }
        queue.apply(&mut world);

        assert!(world.get::<CombatOrder>(htn_entity).is_none());
        assert!(world.get::<CombatOrderSource>(htn_entity).is_none());
        assert!(world.get::<CombatOrder>(player_entity).is_some());
        assert!(world.get::<CombatOrderSource>(player_entity).is_some());
    }
}
