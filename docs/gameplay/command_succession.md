# Command Succession

Command succession is an immediate simulation-authoritative response to `UnitDied`. It preserves a coherent `CommandForest` while keeping persistent squad rosters unchanged.

## Timing

Combat deaths are processed in `FixedUpdate` during `SimulationSet::Cleanup`, after combat and objectives and before the next simulation tick's communications and thinking phases. Debug deaths are processed after debug death commands flush in `Update`; this keeps debug succession functional while simulation is paused and avoids losing short-lived death messages between 20 Hz fixed ticks.

Both paths are registered by the existing `CommandPlugin`. There is no separate succession plugin.

## Deterministic processing

A batch snapshots each deceased unit's pre-mutation command depth and stable `UnitId`. Deaths are processed deepest-first, then by stable `UnitId`. Opaque entity identity is used only as a fallback for malformed or isolated test entities without stable identity.

Every deceased forest node is passed through the atomic `CommandForest::succeed` operation. A nonleader is removed without promotion. If a leader has no eligible successor, the leader is removed, surviving direct children become roots, and the squad has no current leader.

## Eligibility

Leadership follows authored squad roster order. Starting after the deceased current leader, the first candidate is selected only when it:

- is a living `Soldier`;
- has the squad's side; and
- remains a current direct child of the deceased leader.

The roster is never reordered. External subordinates and detached former subordinates cannot succeed the leader.

## Persistent results

A successful rewrite increments the affected squad revision. `CommandStructureChanged` records every successful forest removal or rewrite. `CommandSucceeded` additionally records current-leader death, the optional successor, tick, and resulting squad revision.

Failed atomic forest mutation logs a warning and does not partially update squad leadership, revision, player control, or succession messages. Selection of a deceased player-controlled unit is still cleared independently of topology repair.

## Player death

`PlayerControlledUnit` never transfers. Its death always resolves the mission as Defeat, even when the final hostile dies simultaneously. The ordinary mission outcome transition pauses simulation and emits `MissionEnded` once. A selection pointing to the deceased controlled unit is cleared.

## Plan assumption

A living successor directly assumes an unexpired `AssignedCommandPlan` as doctrine. The plan snapshot, original assigner, original issue tick, rally point, and expiry are preserved; `AssumedCommand` separately records predecessor and assumption time. The predecessor is replaced in `CommandPlanAssignees`. Delegation progress is never copied.

When a plan is inherited, the promoted unit's old subordinate task and HTN execution are superseded so leadership planning can run. Succession never installs a movement or combat order. Expired or absent plans are not inherited.

## Dynamic redelegation

`CommandPlanDelegationProgress` records plan identity, squad revision, and recipient entities only. A plan/revision mismatch clears all recipient progress and drops pending work from the old revision. Existing HTN Hold Line decomposition then rebuilds the entire plan from the current living roster and emits new subordinate directives one at a time in roster order.

Every directive still travels through Outbox and the packet network. Obsolete living recipients can receive newer task-cancellation packets; cancellation removes planner intent and only HTN-sourced execution. The coordinator's own target is recomputed locally by HTN synthesis. No station geometry is retained or compared during invalidation.

## Observability and reset

Assumption and redelegation appear in decision traces. Diagnostics count attempts, assumptions, orphaning, resets, cancellations, and stale packet rejection. Player-facing notices require recent tactical knowledge of both predecessor and successor. Transient notices, diagnostics, and succession messages reset at mission boundaries.
