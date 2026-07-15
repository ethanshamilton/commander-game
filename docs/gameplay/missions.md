# Tactical missions

A tactical mission is a persistent, player-authored intent-bearing plan created
while a scenario is running. It is not an authored scenario and is not a
concrete movement or combat order.

`MissionPlan` stores local plan state, including its area, rally point, optional
expiry, and `MissionAssignees`. `MissionSnapshot` is the copy safe to send in a
packet. A recipient installs `AssignedMission` as an HTN input after packet
validation; it must not directly install `UnitOrder` or `CombatOrder`.

All mission coordinates are meters. `MissionPlan::validate` rejects incompatible
kind/area pairs, non-finite points and radii, degenerate geometry, blank labels,
and expiry at or before plan creation.

Tactical mission IDs are scenario-local and allocated monotonically by
`MissionIdAllocator`. Tactical mission entities are marked `ScenarioScoped`, so
leaving a scenario removes them with its other runtime entities.
