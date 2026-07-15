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

## Assignment

UI and AI call `MissionAssignmentRequested { mission, issuer, assignee }`.
The temporary UI enters **Assign Mission Mode** when a tactical mission is
selected from the M menu; the next selected map unit becomes its assignee. The
request validates that both units are alive soldiers, `issuer` has authority
through `CommandForest`, and the assignee is a squad leader. It adds the assignee
to the plan's `MissionAssignees`, then either installs `AssignedMission` locally
for self-assignment or transmits `PacketPayload::MissionAssignment` through the
issuer's outbox.

Recipients validate the packet origin and mission snapshot again. A valid,
unexpired assignment becomes an HTN input only; it never directly writes a
concrete order. One active assignment is supported per recipient: a greater
`issued_tick` supersedes it, while equal or older packets are ignored.
