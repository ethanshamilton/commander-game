#![doc = include_str!("../../docs/gameplay/diagnostics.md")]

use crate::GameState;
use crate::gameplay::simulation::{SimulationSet, simulation_running};
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use std::time::Instant;

/// Smoothing factor for the per-phase exponential moving average.
/// At 20Hz, α = 0.05 gives roughly a one-second smoothing window.
const PHASE_EMA_ALPHA: f32 = 0.05;

/// Simulation phases in chain order. Indices match `SimulationPerf` phase arrays.
pub const PHASE_NAMES: [&str; PHASE_COUNT] = [
    "Clock",
    "Orders",
    "Movement",
    "SpatialIndex",
    "Sensors",
    "Comms",
    "Reports",
    "Thinking",
    "Combat",
    "Objectives",
    "Cleanup",
];

pub const PHASE_COUNT: usize = 11;

pub struct GameplayDiagnosticsPlugin;

impl Plugin for GameplayDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .init_resource::<SimulationPerf>()
            .add_systems(
                FixedUpdate,
                (
                    begin_simulation_tick.before(SimulationSet::Clock),
                    phase_boundary(0)
                        .after(SimulationSet::Clock)
                        .before(SimulationSet::Orders),
                    phase_boundary(1)
                        .after(SimulationSet::Orders)
                        .before(SimulationSet::Movement),
                    phase_boundary(2)
                        .after(SimulationSet::Movement)
                        .before(SimulationSet::SpatialIndex),
                    phase_boundary(3)
                        .after(SimulationSet::SpatialIndex)
                        .before(SimulationSet::Sensors),
                    phase_boundary(4)
                        .after(SimulationSet::Sensors)
                        .before(SimulationSet::Comms),
                    phase_boundary(5)
                        .after(SimulationSet::Comms)
                        .before(SimulationSet::Reports),
                    phase_boundary(6)
                        .after(SimulationSet::Reports)
                        .before(SimulationSet::Thinking),
                    phase_boundary(7)
                        .after(SimulationSet::Thinking)
                        .before(SimulationSet::Combat),
                    phase_boundary(8)
                        .after(SimulationSet::Combat)
                        .before(SimulationSet::Objectives),
                    phase_boundary(9)
                        .after(SimulationSet::Objectives)
                        .before(SimulationSet::Cleanup),
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
    /// Exponential moving average of each phase's duration, in seconds.
    /// Indexed in chain order; see [`PHASE_NAMES`].
    pub phase_ema_s: [f32; PHASE_COUNT],
    started_at: Option<Instant>,
    last_mark: Option<Instant>,
}

impl Default for SimulationPerf {
    fn default() -> Self {
        Self {
            tick_budget_s: 1.0 / crate::gameplay::simulation::SIMULATION_TICK_HZ as f32,
            last_tick_s: 0.0,
            utilization: 0.0,
            phase_ema_s: [0.0; PHASE_COUNT],
            started_at: None,
            last_mark: None,
        }
    }
}

impl SimulationPerf {
    /// Record the boundary at the end of `phase`, folding the elapsed time
    /// since the previous boundary into that phase's EMA.
    fn mark_phase(&mut self, phase: usize) {
        let now = Instant::now();
        let Some(prev) = self.last_mark.replace(now) else {
            return;
        };

        let dt = (now - prev).as_secs_f32();
        let ema = &mut self.phase_ema_s[phase];
        *ema = if *ema == 0.0 {
            dt
        } else {
            *ema + PHASE_EMA_ALPHA * (dt - *ema)
        };
    }

    /// Phases sorted by average cost, most expensive first.
    pub fn phases_by_cost(&self) -> Vec<(&'static str, f32)> {
        let mut phases: Vec<_> = PHASE_NAMES
            .iter()
            .copied()
            .zip(self.phase_ema_s.iter().copied())
            .collect();
        phases.sort_by(|a, b| b.1.total_cmp(&a.1));
        phases
    }
}

/// Boundary system marking the end of the phase at `phase` index.
fn phase_boundary(phase: usize) -> impl FnMut(ResMut<SimulationPerf>) {
    move |mut perf: ResMut<SimulationPerf>| {
        perf.mark_phase(phase);
    }
}

fn begin_simulation_tick(mut perf: ResMut<SimulationPerf>) {
    let now = Instant::now();
    perf.started_at = Some(now);
    perf.last_mark = Some(now);
}

fn end_simulation_tick(mut perf: ResMut<SimulationPerf>) {
    perf.mark_phase(PHASE_COUNT - 1);

    let Some(started_at) = perf.started_at.take() else {
        return;
    };

    perf.last_tick_s = started_at.elapsed().as_secs_f32();
    perf.utilization = perf.last_tick_s / perf.tick_budget_s;
}
