#![doc = include_str!("../../../docs/gameplay/combat.md")]

pub mod components;
pub mod events;
mod resolution;

use crate::GameState;
use crate::gameplay::simulation::SimulationSet;
use bevy::prelude::*;
use rand::SeedableRng;
use rand::rngs::StdRng;

pub use components::{CombatOrder, CombatState};
pub use events::ResolvedShot;

const DEFAULT_COMBAT_RNG_SEED: u64 = 0xC0_0B_A7;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CombatRng>()
            .add_message::<ResolvedShot>()
            .add_systems(
                FixedUpdate,
                (
                    resolution::terminate_fire_orders,
                    resolution::resolve_combat,
                )
                    .chain()
                    .in_set(SimulationSet::Combat)
                    .run_if(in_state(GameState::ScenarioScreen)),
            );
    }
}

#[derive(Resource)]
pub struct CombatRng(pub StdRng);

impl Default for CombatRng {
    fn default() -> Self {
        Self(StdRng::seed_from_u64(DEFAULT_COMBAT_RNG_SEED))
    }
}
