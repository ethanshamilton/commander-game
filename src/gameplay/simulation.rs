#![doc = include_str!("../../docs/gameplay/simulation.md")]

use crate::GameState;
use crate::actors::units::{Alive, Mobility, Soldier};
use crate::gameplay::orders::MovementOrderSource;
use crate::gameplay::spatial::{BattlefieldPosition, Heading, PositionTarget};
use crate::input::{ActionState, GameAction};
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
                    SimulationSet::SpatialIndex,
                    SimulationSet::Sensors,
                    SimulationSet::Comms,
                    SimulationSet::Reports,
                    SimulationSet::Thinking,
                    SimulationSet::Combat,
                    SimulationSet::Objectives,
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
    SpatialIndex,
    Sensors,
    Comms,
    Reports,
    Thinking,
    Combat,
    Objectives,
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
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub enum MovementOrder {
    MoveTo { target: PositionTarget },
    Hold,
}

pub fn simulation_running(clock: Res<SimulationClock>) -> bool {
    !clock.paused
}

fn toggle_simulation_pause(actions: Res<ActionState>, mut clock: ResMut<SimulationClock>) {
    if actions.just_pressed(GameAction::TogglePause) {
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
            &MovementOrder,
        ),
        (With<Soldier>, With<Alive>),
    >,
) {
    let dt = clock.tick_dt_s * clock.speed;

    for (entity, mut position, mut heading, mobility, order) in &mut units {
        let MovementOrder::MoveTo { target } = *order else {
            continue;
        };

        let offset = target.position_m - position.0;
        let distance_m = offset.length();

        if distance_m <= f32::EPSILON {
            if let Some(arrival_heading) = target.heading_radians {
                heading.0 = arrival_heading;
            }
            commands
                .entity(entity)
                .remove::<(MovementOrder, MovementOrderSource)>();
            continue;
        }

        let max_step_m = mobility.speed.max(0) as f32 * dt;
        if distance_m <= max_step_m {
            position.0 = target.position_m;
            if let Some(arrival_heading) = target.heading_radians {
                heading.0 = arrival_heading;
            }
            commands
                .entity(entity)
                .remove::<(MovementOrder, MovementOrderSource)>();
        } else {
            let direction = offset / distance_m;
            position.0 += direction * max_step_m;
            heading.0 = direction.to_angle();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::units::{Rank, Role};
    use bevy::ecs::system::RunSystemOnce;

    fn unit_at_target(world: &mut World, heading: f32, arrival_heading: Option<f32>) -> Entity {
        world
            .spawn((
                Soldier {
                    rank: Rank::Private,
                    role: Role::Rifleman,
                },
                Alive,
                Mobility { speed: 1 },
                BattlefieldPosition(Vec2::ZERO),
                Heading(heading),
                MovementOrder::MoveTo {
                    target: PositionTarget::new(Vec2::ZERO, arrival_heading),
                },
                MovementOrderSource::htn(),
            ))
            .id()
    }

    #[test]
    fn arrival_heading_is_applied_before_move_order_completes() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        let entity = unit_at_target(&mut world, 0.0, Some(1.25));

        world.run_system_once(move_units).unwrap();
        world.flush();

        assert_eq!(world.get::<Heading>(entity).unwrap().0, 1.25);
        assert!(world.get::<MovementOrder>(entity).is_none());
        assert!(world.get::<MovementOrderSource>(entity).is_none());
    }

    #[test]
    fn position_only_arrival_preserves_existing_heading() {
        let mut world = World::new();
        world.insert_resource(SimulationClock::default());
        let entity = unit_at_target(&mut world, 0.75, None);

        world.run_system_once(move_units).unwrap();
        world.flush();

        assert_eq!(world.get::<Heading>(entity).unwrap().0, 0.75);
    }
}
