# Objectives

Scenario objectives evaluate scenario-level victory and defeat conditions.

The objective system intentionally evaluates **ground truth**, not player tactical knowledge. A
unit can satisfy a victory condition even if the player has not yet received a report confirming it.
Player-facing awareness of that outcome can become a separate intel/UI problem later.

Current semantics:

- defeat conditions are evaluated before victory conditions
- any satisfied defeat condition ends the scenario in defeat
- all victory conditions must be satisfied to end the scenario in victory
- simultaneous victory and defeat resolves to defeat
- scenario end pauses the simulation clock

Conditions are scenario data, while `ScenarioOutcome` is runtime state.
