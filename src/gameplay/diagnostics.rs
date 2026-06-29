#![doc = include_str!("../../docs/gameplay/diagnostics.md")]

use crate::GameState;
use crate::gameplay::simulation::{SimulationSet, simulation_running};
use bevy::prelude::*;
use std::time::Instant;

pub struct GameplayDiagnosticsPlugin;

impl Plugin for GameplayDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimulationPerf>().add_systems(
            FixedUpdate,
            (
                begin_simulation_tick.before(SimulationSet::Clock),
                end_simulation_tick.after(SimulationSet::Cleanup),
            )
                .run_if(in_state(GameState::MissionScreen))
                .run_if(simulation_running),
        );
    }
}

#[derive(Resource, Debug)]
pub struct SimulationPerf {
    pub tick_budget_s: f32,
    pub last_tick_s: f32,
    pub utilization: f32,
    started_at: Option<Instant>,
}

impl Default for SimulationPerf {
    fn default() -> Self {
        Self {
            tick_budget_s: 1.0 / crate::gameplay::simulation::SIMULATION_TICK_HZ as f32,
            last_tick_s: 0.0,
            utilization: 0.0,
            started_at: None,
        }
    }
}

fn begin_simulation_tick(mut perf: ResMut<SimulationPerf>) {
    perf.started_at = Some(Instant::now());
}

fn end_simulation_tick(mut perf: ResMut<SimulationPerf>) {
    let Some(started_at) = perf.started_at.take() else {
        return;
    };

    perf.last_tick_s = started_at.elapsed().as_secs_f32();
    perf.utilization = perf.last_tick_s / perf.tick_budget_s;
}
