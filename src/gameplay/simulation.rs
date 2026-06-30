#![doc = include_str!("../../docs/gameplay/simulation.md")]

use crate::GameState;
use crate::gameplay::components::{BattlefieldPosition, Heading};
use crate::units::{Alive, Mobility, Soldier};
use bevy::prelude::*;

pub const SIMULATION_TICK_HZ: f64 = 20.0;

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimulationClock>()
            .insert_resource(Time::<Fixed>::from_hz(SIMULATION_TICK_HZ))
            .configure_sets(
                FixedUpdate,
                (
                    SimulationSet::Clock,
                    SimulationSet::Orders,
                    SimulationSet::Movement,
                    SimulationSet::Sensors,
                    SimulationSet::Comms,
                    SimulationSet::Reports,
                    SimulationSet::Combat,
                    SimulationSet::Cleanup,
                )
                    .chain()
                    .run_if(in_state(GameState::MissionScreen))
                    .run_if(simulation_running),
            )
            .add_systems(
                Update,
                toggle_simulation_pause.run_if(in_state(GameState::MissionScreen)),
            )
            .add_systems(
                FixedUpdate,
                (
                    advance_simulation_clock.in_set(SimulationSet::Clock),
                    move_units.in_set(SimulationSet::Movement),
                ),
            );
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimulationSet {
    Clock,
    Orders,
    Movement,
    Comms,
    Sensors,
    Reports,
    Combat,
    Cleanup,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct SimulationClock {
    pub paused: bool,
    pub tick: u64,
    pub elapsed_s: f32,
    pub tick_dt_s: f32,
    pub speed: f32,
}

impl Default for SimulationClock {
    fn default() -> Self {
        Self {
            paused: false,
            tick: 0,
            elapsed_s: 0.0,
            tick_dt_s: (1.0 / SIMULATION_TICK_HZ) as f32,
            speed: 1.0,
        }
    }
}

#[allow(dead_code)]
#[derive(Component, Debug, Clone, Copy)]
pub enum UnitOrder {
    MoveTo { destination_m: Vec2 },
    Hold,
}

pub fn simulation_running(clock: Res<SimulationClock>) -> bool {
    !clock.paused
}

fn toggle_simulation_pause(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut clock: ResMut<SimulationClock>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        clock.paused = !clock.paused;
    }
}

fn advance_simulation_clock(mut clock: ResMut<SimulationClock>) {
    let dt = clock.tick_dt_s * clock.speed;
    clock.tick += 1;
    clock.elapsed_s += dt;
}

fn move_units(
    mut commands: Commands,
    clock: Res<SimulationClock>,
    mut units: Query<
        (
            Entity,
            &mut BattlefieldPosition,
            &mut Heading,
            &Mobility,
            &UnitOrder,
        ),
        (With<Soldier>, With<Alive>),
    >,
) {
    let dt = clock.tick_dt_s * clock.speed;

    for (entity, mut position, mut heading, mobility, order) in &mut units {
        let UnitOrder::MoveTo { destination_m } = *order else {
            continue;
        };

        let offset = destination_m - position.0;
        let distance_m = offset.length();

        if distance_m <= f32::EPSILON {
            commands.entity(entity).remove::<UnitOrder>();
            continue;
        }

        let max_step_m = mobility.speed.max(0) as f32 * dt;
        if distance_m <= max_step_m {
            position.0 = destination_m;
            commands.entity(entity).remove::<UnitOrder>();
        } else {
            let direction = offset / distance_m;
            position.0 += direction * max_step_m;
            heading.0 = direction.to_angle();
        }
    }
}
