# Tactical plans

A tactical plan is a persistent, player-authored intent-bearing plan created
while a mission is running. It is not an authored mission and is not a
concrete movement or combat order.

`CommandPlan` stores local plan state, including its area, rally point, optional
expiry, and `CommandPlanAssignees`. `CommandPlanSnapshot` is the copy safe to send in a
packet. A recipient installs `AssignedCommandPlan` as an HTN input after packet
validation; it must not directly install `MovementOrder` or `CombatOrder`.

All plan coordinates are meters. `CommandPlan::validate` rejects incompatible
kind/area pairs, non-finite points and radii, degenerate geometry, blank labels,
and expiry at or before plan creation.

Tactical plan IDs are mission-local and allocated monotonically by
`CommandPlanIdAllocator`. Tactical plan entities are marked `MissionScoped`, so
leaving a mission removes them with its other runtime entities.

## Assignment

UI and AI call `CommandPlanAssignmentRequested { plan, issuer, assignee }`.
The temporary UI enters **Assign Plan Mode** when a tactical plan is
selected from the M menu; the next selected map unit becomes its assignee. The
request validates that both units are alive soldiers, `issuer` has authority
through `CommandForest`, and the assignee is a squad leader. It adds the assignee
to the plan's `CommandPlanAssignees`, then either installs `AssignedCommandPlan` locally
for self-assignment or transmits `PacketPayload::CommandPlanAssignment` through the
issuer's outbox.

Recipients validate the packet origin and plan snapshot again. A valid,
unexpired assignment becomes an HTN input only; it never directly writes a
concrete order. One active assignment is supported per recipient: a greater
`issued_tick` supersedes it, while equal or older packets are ignored.

## Leader decomposition

All infantry use one domain containing both ordinary soldier and conditional
leadership behavior. An assigned Hold Line plan activates command behavior
for its current coordinator. The coordinator and all living direct command
members are jointly assigned evenly spaced formation stations; the coordinator
receives a central slot and delegates the other slots one per planning cycle.
The same decomposition generates a wedge at the plan rally point, reserves
its anchor for the coordinator, and bundles each member's unique fallback
station into that member's Hold Station directive. Hold and fallback stations
are `PositionTarget` poses carrying the shared formation heading, so members
face back toward the original line after arriving.
`CommandPlanDelegationProgress` records plan identity, squad revision, and recipients accepted for transmission so replanning is idempotent. A plan or revision mismatch clears all recipient progress and stale pending work; HTN decomposition then rebuilds and reissues the full living roster without retaining or comparing old station geometry. The HTN emits revision-stamped `PendingTaskAssignment`; a gameplay bridge revalidates it and turns it into `PacketPayload::TaskAssignment` without writing concrete orders. After delegation, the coordinator moves to and holds its own recomputed station. Coordinator is a transient command relationship, not a rank-specific domain.

Recipients validate task authority, geometry, expiry, and `issued_tick` before
installing `AssignedTask`. Soldier and squad-leader HTN domains share assigned-
task behavior: units move to their station with an HTN-sourced `MovementOrder`, then
hold there. Survival and fresh-contact engagement remain higher priorities;
displacement or task replacement invalidates the running task step and causes
replanning.

## Expiration fallback

At `tick >= expires_at`, normal plan/task execution stops. The coordinator
moves to the rally point while each recipient moves to the unique wedge station
that the coordinator sent with its original task directive. The squad therefore
retains its fallback formation even without working comms. HTN moves each unit
to its station and holds there; survival and fresh-contact engagement still
outrank regrouping. Expired assignment components are retained
for their rally data until a newer assignment supersedes them, while expired
player-authored plans reject new assignment requests. If stale plan
and task components coexist, the one with the newer `issued_tick` controls the
fallback.
