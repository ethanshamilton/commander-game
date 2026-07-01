use bevy::prelude::*;

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct CombatState {
    pub next_fire_tick: u64,
}

#[derive(Component, Debug, Clone, Copy)]
pub enum CombatOrder {
    FireAt { target: Entity },
    HoldFire,
}
