#![doc = include_str!("../../docs/gameplay/objectives.md")]

use crate::GameState;
use crate::actors::units::{Alive, Allegiance, Side, Soldier};
use crate::gameplay::simulation::{SimulationClock, SimulationSet};
use crate::player::control::PlayerControl;
use bevy::prelude::*;

pub struct ObjectivesPlugin;

impl Plugin for ObjectivesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScenarioObjectiveSet>()
            .init_resource::<ScenarioOutcome>()
            .add_message::<ScenarioEnded>()
            .add_systems(
                FixedUpdate,
                evaluate_scenario_outcome
                    .in_set(SimulationSet::Objectives)
                    .run_if(in_state(GameState::ScenarioScreen)),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScenarioCondition {
    /// Victory/defeat condition where every unit not on the player's side is dead.
    AllHostilesEliminated,
    /// Victory/defeat condition where every unit on the player's side is dead.
    AllFriendliesEliminated,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct ScenarioObjectiveSet {
    pub victory_conditions: Vec<ScenarioCondition>,
    pub defeat_conditions: Vec<ScenarioCondition>,
}

impl ScenarioObjectiveSet {
    pub fn from_slices(
        victory_conditions: &[ScenarioCondition],
        defeat_conditions: &[ScenarioCondition],
    ) -> Self {
        Self {
            victory_conditions: victory_conditions.to_vec(),
            defeat_conditions: defeat_conditions.to_vec(),
        }
    }
}

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScenarioOutcome {
    #[default]
    InProgress,
    Victory,
    Defeat,
}

impl ScenarioOutcome {
    pub fn is_finished(self) -> bool {
        self != Self::InProgress
    }
}

#[allow(dead_code)]
#[derive(Message, Debug, Clone, Copy)]
pub struct ScenarioEnded {
    pub outcome: ScenarioOutcome,
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

fn evaluate_scenario_outcome(
    objectives: Res<ScenarioObjectiveSet>,
    mut outcome: ResMut<ScenarioOutcome>,
    mut clock: ResMut<SimulationClock>,
    control: Res<PlayerControl>,
    units: Query<(&Allegiance, Option<&Alive>), With<Soldier>>,
    mut scenario_ended: MessageWriter<ScenarioEnded>,
) {
    if outcome.is_finished() {
        return;
    }

    let counts = side_counts(&units, control.side);
    let next = next_outcome(&objectives, counts);

    if next == ScenarioOutcome::InProgress {
        return;
    }

    *outcome = next;
    clock.paused = true;
    scenario_ended.write(ScenarioEnded { outcome: next });
    info!("Scenario ended: {next:?}");
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

fn next_outcome(objectives: &ScenarioObjectiveSet, counts: SideCounts) -> ScenarioOutcome {
    // Avoid vacuous truth while a scenario is empty or still being initialized.
    if counts.total_units() == 0 {
        return ScenarioOutcome::InProgress;
    }

    if any_condition_met(&objectives.defeat_conditions, counts) {
        return ScenarioOutcome::Defeat;
    }

    if !objectives.victory_conditions.is_empty()
        && objectives
            .victory_conditions
            .iter()
            .all(|condition| condition_met(*condition, counts))
    {
        return ScenarioOutcome::Victory;
    }

    ScenarioOutcome::InProgress
}

fn any_condition_met(conditions: &[ScenarioCondition], counts: SideCounts) -> bool {
    conditions
        .iter()
        .any(|condition| condition_met(*condition, counts))
}

fn condition_met(condition: ScenarioCondition, counts: SideCounts) -> bool {
    match condition {
        ScenarioCondition::AllHostilesEliminated => {
            counts.hostile_total > 0 && counts.hostile_alive == 0
        }
        ScenarioCondition::AllFriendliesEliminated => {
            counts.friendly_total > 0 && counts.friendly_alive == 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn objectives(
        victory_conditions: Vec<ScenarioCondition>,
        defeat_conditions: Vec<ScenarioCondition>,
    ) -> ScenarioObjectiveSet {
        ScenarioObjectiveSet {
            victory_conditions,
            defeat_conditions,
        }
    }

    #[test]
    fn all_hostiles_eliminated_requires_hostiles_to_exist_and_be_dead() {
        let condition = ScenarioCondition::AllHostilesEliminated;
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
        let condition = ScenarioCondition::AllFriendliesEliminated;
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
            vec![ScenarioCondition::AllHostilesEliminated],
            vec![ScenarioCondition::AllFriendliesEliminated],
        );
        let counts = SideCounts {
            friendly_total: 1,
            friendly_alive: 0,
            hostile_total: 1,
            hostile_alive: 0,
        };

        assert_eq!(next_outcome(&objectives, counts), ScenarioOutcome::Defeat);
    }

    #[test]
    fn victory_requires_all_victory_conditions() {
        let objectives = objectives(
            vec![
                ScenarioCondition::AllHostilesEliminated,
                ScenarioCondition::AllFriendliesEliminated,
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
            ScenarioOutcome::InProgress
        );
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
            ScenarioOutcome::InProgress
        );
    }
}
