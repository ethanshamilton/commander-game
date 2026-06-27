use bevy::prelude::*;

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, _app: &mut App) {
        // Stub. Simulation systems will live here: orders, movement, combat, sensors, etc.
    }
}
