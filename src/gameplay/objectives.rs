#![doc = include_str!("../../docs/gameplay/objectives.md")]

use crate::GameState;
use crate::actors::units::{Alive, Allegiance, Side, Soldier};
use crate::gameplay::simulation::{SimulationClock, SimulationSet};
use crate::player::control::PlayerControl;
use crate::player::knowledge::PlayerControlledUnit;
use bevy::prelude::*;

pub struct ObjectivesPlugin;

impl Plugin for ObjectivesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MissionObjectiveSet>()
            .init_resource::<MissionOutcome>()
            .add_message::<MissionEnded>()
            .add_systems(
                FixedUpdate,
                evaluate_mission_outcome
                    .in_set(SimulationSet::Objectives)
                    .run_if(in_state(GameState::MissionScreen)),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionCondition {
    /// Victory/defeat condition where every unit not on the player's side is dead.
    AllHostilesEliminated,
    /// Victory/defeat condition where every unit on the player's side is dead.
    AllFriendliesEliminated,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct MissionObjectiveSet {
    pub victory_conditions: Vec<MissionCondition>,
    pub defeat_conditions: Vec<MissionCondition>,
}

impl MissionObjectiveSet {
    pub fn from_slices(
        victory_conditions: &[MissionCondition],
        defeat_conditions: &[MissionCondition],
    ) -> Self {
        Self {
            victory_conditions: victory_conditions.to_vec(),
            defeat_conditions: defeat_conditions.to_vec(),
        }
    }
}

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MissionOutcome {
    #[default]
    InProgress,
    Victory,
    Defeat,
}

impl MissionOutcome {
    pub fn is_finished(self) -> bool {
        self != Self::InProgress
    }
}

#[allow(dead_code)]
#[derive(Message, Debug, Clone, Copy)]
pub struct MissionEnded {
    pub outcome: MissionOutcome,
}

#[derive(Debug, Clone, Copy, Default)]
struct SideCounts {
    friendly_total: usize,
    friendly_alive: usize,
    hostile_total: usize,
    hostile_alive: usize,
}

impl SideCounts {
    fn add_unit(&mut self, side: Side, player_side: Side, alive: bool) {
        if side == player_side {
            self.friendly_total += 1;
            if alive {
                self.friendly_alive += 1;
            }
        } else {
            self.hostile_total += 1;
            if alive {
                self.hostile_alive += 1;
            }
        }
    }

    fn total_units(self) -> usize {
        self.friendly_total + self.hostile_total
    }
}

fn evaluate_mission_outcome(
    objectives: Res<MissionObjectiveSet>,
    mut outcome: ResMut<MissionOutcome>,
    mut clock: ResMut<SimulationClock>,
    control: Res<PlayerControl>,
    units: Query<(&Allegiance, Option<&Alive>), With<Soldier>>,
    dead_player_unit: Query<(), (With<PlayerControlledUnit>, Without<Alive>)>,
    mut mission_ended: MessageWriter<MissionEnded>,
) {
    if outcome.is_finished() {
        return;
    }

    // Player-command death has precedence over ordinary conditions, including
    // simultaneous elimination of the final hostile unit.
    if !dead_player_unit.is_empty() {
        transition_mission_outcome(
            MissionOutcome::Defeat,
            &mut outcome,
            &mut clock,
            &mut mission_ended,
        );
        return;
    }

    let counts = side_counts(&units, control.side);
    let next = next_outcome(&objectives, counts);
    transition_mission_outcome(next, &mut outcome, &mut clock, &mut mission_ended);
}

pub(crate) fn transition_mission_outcome(
    next: MissionOutcome,
    outcome: &mut MissionOutcome,
    clock: &mut SimulationClock,
    mission_ended: &mut MessageWriter<MissionEnded>,
) -> bool {
    if next == MissionOutcome::InProgress || outcome.is_finished() {
        return false;
    }

    *outcome = next;
    clock.paused = true;
    mission_ended.write(MissionEnded { outcome: next });
    info!("Mission ended: {next:?}");
    true
}

fn side_counts(
    units: &Query<(&Allegiance, Option<&Alive>), With<Soldier>>,
    player_side: Side,
) -> SideCounts {
    let mut counts = SideCounts::default();

    for (allegiance, alive) in units {
        counts.add_unit(allegiance.side, player_side, alive.is_some());
    }

    counts
}

fn next_outcome(objectives: &MissionObjectiveSet, counts: SideCounts) -> MissionOutcome {
    // Avoid vacuous truth while a mission is empty or still being initialized.
    if counts.total_units() == 0 {
        return MissionOutcome::InProgress;
    }

    if any_condition_met(&objectives.defeat_conditions, counts) {
        return MissionOutcome::Defeat;
    }

    if !objectives.victory_conditions.is_empty()
        && objectives
            .victory_conditions
            .iter()
            .all(|condition| condition_met(*condition, counts))
    {
        return MissionOutcome::Victory;
    }

    MissionOutcome::InProgress
}

fn any_condition_met(conditions: &[MissionCondition], counts: SideCounts) -> bool {
    conditions
        .iter()
        .any(|condition| condition_met(*condition, counts))
}

fn condition_met(condition: MissionCondition, counts: SideCounts) -> bool {
    match condition {
        MissionCondition::AllHostilesEliminated => {
            counts.hostile_total > 0 && counts.hostile_alive == 0
        }
        MissionCondition::AllFriendliesEliminated => {
            counts.friendly_total > 0 && counts.friendly_alive == 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::units::{Rank, Role};
    use bevy::ecs::system::RunSystemOnce;

    fn objectives(
        victory_conditions: Vec<MissionCondition>,
        defeat_conditions: Vec<MissionCondition>,
    ) -> MissionObjectiveSet {
        MissionObjectiveSet {
            victory_conditions,
            defeat_conditions,
        }
    }

    #[test]
    fn all_hostiles_eliminated_requires_hostiles_to_exist_and_be_dead() {
        let condition = MissionCondition::AllHostilesEliminated;
        assert!(!condition_met(condition, SideCounts::default()));
        assert!(!condition_met(
            condition,
            SideCounts {
                hostile_total: 2,
                hostile_alive: 1,
                ..default()
            }
        ));
        assert!(condition_met(
            condition,
            SideCounts {
                hostile_total: 2,
                hostile_alive: 0,
                ..default()
            }
        ));
    }

    #[test]
    fn all_friendlies_eliminated_requires_friendlies_to_exist_and_be_dead() {
        let condition = MissionCondition::AllFriendliesEliminated;
        assert!(!condition_met(condition, SideCounts::default()));
        assert!(!condition_met(
            condition,
            SideCounts {
                friendly_total: 3,
                friendly_alive: 1,
                ..default()
            }
        ));
        assert!(condition_met(
            condition,
            SideCounts {
                friendly_total: 3,
                friendly_alive: 0,
                ..default()
            }
        ));
    }

    #[test]
    fn defeat_takes_precedence_when_victory_and_defeat_are_simultaneously_true() {
        let objectives = objectives(
            vec![MissionCondition::AllHostilesEliminated],
            vec![MissionCondition::AllFriendliesEliminated],
        );
        let counts = SideCounts {
            friendly_total: 1,
            friendly_alive: 0,
            hostile_total: 1,
            hostile_alive: 0,
        };

        assert_eq!(next_outcome(&objectives, counts), MissionOutcome::Defeat);
    }

    #[test]
    fn victory_requires_all_victory_conditions() {
        let objectives = objectives(
            vec![
                MissionCondition::AllHostilesEliminated,
                MissionCondition::AllFriendliesEliminated,
            ],
            vec![],
        );
        let counts = SideCounts {
            friendly_total: 1,
            friendly_alive: 1,
            hostile_total: 1,
            hostile_alive: 0,
        };

        assert_eq!(
            next_outcome(&objectives, counts),
            MissionOutcome::InProgress
        );
    }

    #[test]
    fn player_unit_death_beats_simultaneous_hostile_elimination_and_emits_once() {
        let mut world = World::new();
        world.insert_resource(objectives(
            vec![MissionCondition::AllHostilesEliminated],
            vec![],
        ));
        world.insert_resource(MissionOutcome::InProgress);
        world.insert_resource(SimulationClock::default());
        world.insert_resource(PlayerControl::default());
        world.init_resource::<Messages<MissionEnded>>();
        world.spawn((
            Soldier {
                rank: Rank::Sergeant,
                role: Role::Rifleman,
            },
            Allegiance { side: Side::Blue },
            PlayerControlledUnit,
        ));
        world.spawn((
            Soldier {
                rank: Rank::Private,
                role: Role::Rifleman,
            },
            Allegiance { side: Side::Red },
        ));

        world.run_system_once(evaluate_mission_outcome).unwrap();
        world.run_system_once(evaluate_mission_outcome).unwrap();

        assert_eq!(*world.resource::<MissionOutcome>(), MissionOutcome::Defeat);
        assert!(world.resource::<SimulationClock>().paused);
        assert_eq!(world.resource::<Messages<MissionEnded>>().len(), 1);
    }

    #[test]
    fn empty_victory_conditions_do_not_autowin() {
        let objectives = objectives(vec![], vec![]);
        let counts = SideCounts {
            friendly_total: 1,
            friendly_alive: 1,
            hostile_total: 1,
            hostile_alive: 1,
        };

        assert_eq!(
            next_outcome(&objectives, counts),
            MissionOutcome::InProgress
        );
    }
}
