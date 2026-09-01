# NOW Plan: Command Succession and Dynamic Redelegation

## Roadmap item

> **COMMAND:** Command succession and dynamic redelegation.

## Goal

When a commander dies or otherwise becomes unable to command, preserve a valid chain of command, promote a deterministic successor, transfer command responsibility, and make affected plans/tasks re-decompose for the new structure.

The first implementation is simulation-authoritative and immediate. It establishes correct mechanics before later modeling delays in learning that a leader died.

## Current behavior and failure modes

`CommandForest` already provides a cycle-safe individual superior/subordinate forest. Mission data initializes it from stable `UnitId`s.

However:

- `kill_unit` strips active capabilities but does not update `CommandForest`.
- `CommandForest::remove_unit` simply orphans direct children.
- dead commanders remain valid ancestors for `can_command` because authority checks do not inspect life state;
- an `AssignedCommandPlan` remains attached to the dead coordinator;
- surviving members retain tasks from the old coordinator, but nobody monitors or revises them;
- Hold Line decomposition excludes dead members but `CommandPlanDelegationProgress` can suppress needed re-delegation to survivors;
- if the dead unit had `PlayerControlledUnit`, selection/order code loses its single issuer;
- no event/trace/UI record explains a command transition.

## V1 policy decisions

### Trigger

Succession begins on an explicit lifecycle transition, not by scanning every frame for missing components. Add a `UnitDied` message emitted exactly once by the authoritative death path. Make `kill_unit` idempotent so combat and debug-kill cannot emit duplicate deaths.

Later causes such as incapacitation, surrender, dismissal, and communications isolation can emit a more general `CommandUnavailable` event. Death is the NOW scope.

### Eligible successor

Succession is defined by the commander's persistent squad roster, not inferred from every direct subordinate in `CommandForest`. The authored roster is ordered:

```text
[current leader, first successor, second successor, ...]
```

Promotion changes `Squad.current_leader`; it never reorders the roster. Starting after the deceased leader's roster position, choose the first member that:

- has `Soldier + Alive`;
- has the same `Side` as the squad;
- is still a direct child of the deceased leader in the pre-transition forest.

The direct-child requirement is a V1 consistency check: every living nonleader squad member is initially a direct subordinate of its leader. External direct subordinates, such as other squad leaders, are never succession candidates unless they also belong to the deceased commander's squad.

Roster position is the complete deterministic policy. Do not add a parallel `SuccessionPriority`, rank tiebreaker, or entity-order rule.

### Tree rewrite

Given parent `P`, dead commander `D`, chosen successor `S`, and other direct children `C*`:

```text
before: P -> D -> [S, C1, C2]
after:  P -> S -> [C1, C2]
```

If `D` was a root, `S` becomes a root. If no eligible successor exists, remove `D` and orphan its surviving children, matching the conservative current behavior. A dead leaf is simply removed.

Only direct children move. Existing deeper subtrees remain attached to their immediate leaders.

### Authority timing

The rewrite occurs in `SimulationSet::Cleanup`, after Combat has resolved deaths and before the next tick's Comms/Thinking phases. Therefore:

- packets accepted earlier in the death tick use the old valid structure;
- the new structure governs all packets and planning on the following tick;
- no system sees a half-mutated forest.

### Intent inheritance

If `D` has an active `AssignedCommandPlan`, `S` inherits a copy as an assumed command responsibility. Preserve plan ID, snapshot, original assigner, original issue tick, and expiry. Record assumption separately:

```rust
#[derive(Component)]
pub struct AssumedCommand {
    pub predecessor: Entity,
    pub assumed_tick: u64,
    pub cause: CommandAssumptionCause,
}
```

The succession system is allowed to install this directly because assumption of an existing lawful mission is doctrine, not a new remote order. It must not create a concrete movement/combat order.

Do **not** copy the predecessor's delegation progress. The successor recomputes against the squad's living roster.

Existing subordinate `AssignedTask`s remain valid until superseded, cancelled, or expired. On the next planning pass, the successor emits revised tasks to affected living squad members in roster order. Newer assignment stamps supersede the predecessor's tasks.

### Player command node

`PlayerControlledUnit` never transfers. Its death always resolves the mission as Defeat, including simultaneous elimination of the final hostile. Selection of the dead controlled unit is cleared, while dead-unit knowledge/history remains intact.

Command succession is exercised through non-player-controlled squad leaders (the multi-squad tutorial fixture), not by transferring the player's embodied command node.

## CommandForest API changes

Keep topology mutation inside `CommandForest`; external systems should not compose several `set_superior` calls and temporarily violate invariants.

```rust
pub struct SuccessionOutcome {
    pub deceased: Entity,
    pub old_superior: Option<Entity>,
    pub successor: Option<Entity>,
    pub transferred_subordinates: Vec<Entity>,
}

impl CommandForest {
    pub fn succeed(
        &mut self,
        deceased: Entity,
        successor: Option<Entity>,
    ) -> Result<SuccessionOutcome, CommandMutationError>;

    pub fn validate(&self) -> Result<(), CommandForestInvariantError>;
}
```

`succeed` validates that `successor` is currently a direct child before mutating. Build the desired rewrite first, then commit it. Failed validation leaves the forest unchanged.

Make roots and subordinate iteration deterministic where observable. Either sort returned entities by stable IDs outside the forest or store an explicit authored order; do not globally sort by opaque `Entity` unless only used as fallback.

Update authority APIs so callers dealing with orders use a helper that checks both topology and living endpoints. Keep topology-only `can_command` if useful, but name the distinction clearly:

```rust
can_command_in_forest(issuer, recipient)
can_issue_command(issuer, recipient, living_query)
```

Packet consumers and assignment request handlers use the life-aware form.

## Dynamic redelegation

Succession is one cause of a broader rule: a plan coordinator must revise delegation whenever its effective command membership changes.

### Squad revision invalidation

Keep invalidation explicit and small. `Squad.revision` increments whenever membership availability or leadership changes. Delegation progress records the squad revision it was computed against; a mismatch clears delegated recipients and recomputes against the living roster.

For the first death/succession slice, the lifecycle system may directly clear affected `CommandPlanDelegationProgress`. Add revision comparison when squad mutation expands beyond death. Do not add a participant-vector signature or several overlapping squad-change events preemptively.

After invalidation:

1. clear all accepted subordinate recipients;
2. recompute the full plan from the living roster in authored order;
3. reissue tasks to every living subordinate one at a time through comms;
4. explicitly cancel obsolete tasks where the recipient is still alive and lawfully reachable; and
5. update the coordinator's own target.

Do not retain or compare old station geometry. Full deterministic rebuild is preferred over changed-only optimization.

A single `CommandSucceeded` message carries historical old/new leadership data for UI and traces. Current squad truth remains persistent in the `Squad` component; ordinary consumers use that state rather than reconstructing it from events.

### In-flight packets

Old packets may arrive after succession. Required recipient checks:

- origin had authority for the recipient under the relevant command epoch;
- assignment stamp is newer than the installed one;
- plan/task is not cancelled or expired.

For V1, the current forest plus strict newer revision is sufficient because succession occurs between ticks and the successor reissues on the next tick. If tests expose legitimate predecessor packets being rejected after topology changes, add a short-lived `CommandEpochHistory` rather than weakening authority globally.

A packet from the dead predecessor that was not accepted before restructuring should normally be consumed and rejected. The successor's new directive is authoritative.

## Succession processing system

Keep succession implementation in `src/gameplay/command_succession.rs`, registered by the existing `CommandPlugin`; do not add a separate plugin.

Per `UnitDied`:

1. Snapshot dead unit's side, parent, direct children, assigned plan, and control marker.
2. Resolve the dead unit's `MemberOfSquad` and read candidates from the ordered roster.
3. Choose the first eligible member after the deceased leader with a pure helper.
4. Call one atomic `CommandForest::succeed` mutation.
5. Remove command-only components from the deceased (`AssignedCommandPlan`, pending delegation, progress, assumed-command marker).
6. If a successor exists:
   - install inherited `AssignedCommandPlan` if applicable;
   - reset/generate delegation progress;
   - add `AssumedCommand`;
   - never transfer `PlayerControlledUnit`; its death is handled as Defeat.
7. Increment the squad revision and emit `CommandSucceeded`.
8. Add a decision/command trace record to the successor.
9. Let deferred ECS commands flush before the next simulation tick.

Multiple deaths in one combat pass require care. Sort death events by pre-death depth, deepest first, then stable unit ID. Re-evaluate eligibility against the already updated forest for each event. Test simultaneous death of leader and designated successor.

## UI and observability

Minimal NOW UX:

- selected-unit panel shows direct superior and direct subordinates from player knowledge;
- successor gets a visible `Assumed command from <name>` status;
- HUD toast/log reports `Leader X killed; Y assumed command`;
- command relation overlay updates from `Changed<CommandForest>` immediately;
- decision trace records predecessor, reason, inherited plan ID, and redelegation revision;
- diagnostics count succession attempts, successful assumptions, orphaned commands, and rejected stale packets.

Do not reveal an unreported enemy succession to the player. Enemy topology can update in simulation, but player-facing messages/overlays require ordinary intel visibility.

## Implementation sequence

### DONE C1 — Lifecycle event and idempotent death

- Add `UnitDied { entity, tick, cause }`.
- Make combat and debug kill use one transition API.
- Return early if already dead/not alive.
- Add lifecycle tests proving one event and capability stripping.

### DONE C2 — Minimal squad organization

- Add ordered `SquadDefinition`s to mission authoring.
- Spawn persistent `Squad` entities and install `MemberOfSquad` reverse links.
- Derive initial squad-internal forest links from each roster.
- Validate unique/nonempty squads, unique membership, known units, and same-side rosters.
- Make formation decomposition preserve authored roster order.
- Implement and unit-test ordered successor selection.

### DONE C3 — Atomic forest succession

- Implement `CommandForest::succeed` and invariant validation.
- Cover root, middle node, leaf, no successor, malformed successor, and cycle safety.
- Make authority call sites life-aware.

### DONE C4 — Runtime succession

- Add plugin/system in `SimulationSet::Cleanup`.
- Transfer player control and selected unit.
- Emit structure/succession messages.
- Handle multiple deaths deterministically.

**Done when:** debug-killing a squad leader promotes the authored successor before the next simulation tick and all surviving squad members have the new direct superior.

### DONE C5 — Plan assumption

- Transfer active plan intent without copying stale execution progress.
- Add `AssumedCommand` and trace/UI status.
- Preserve expiry/fallback data.
- Ensure a successor with no inherited plan falls back to ordinary infantry behavior.

### DONE C6 — Dynamic redelegation

- Replace `delegated_to`-only invalidation with squad revision-aware progress.
- Clear progress and rebuild/reissue the full plan after any command-membership revision.
- Add cancellation for obsolete live recipients.
- Verify concrete orders remain HTN-sourced.

### DONE C7 — Hardening

- Simultaneous leader/successor death.
- Root player commander death produces Defeat with no control transfer.
- Dead packet origin, delayed predecessor packet, comms-isolated successor.
- Successor dies during assumption.
- Mission exit/reset.
- Docs and diagnostics.

## Likely file changes

- New: `src/gameplay/command_succession.rs`, `docs/gameplay/command_succession.md`.
- Edit: `src/gameplay/command.rs`, `lifecycle.rs`, `combat/resolution.rs`, `debug_powers.rs`, `mod.rs`, `simulation.rs`.
- Edit: `src/actors/units.rs`, `spawning.rs`, `src/missions.rs`.
- Edit: `src/gameplay/command_plans.rs`, `packets.rs`.
- Edit: `src/ai/htn/synthesis.rs`, `leader.rs`, `trace.rs`.
- Edit: player selection/control and mission HUD/unit panel.

## Test scenarios

1. **Middle-node death:** `P -> D -> [S,A,B]` becomes `P -> S -> [A,B]`.
2. **Root death:** `D -> [S,A]` becomes root `S -> [A]`.
3. **Leaf death:** leaf disappears from parent, no promotion.
4. **No eligible candidate:** children become roots and no invalid references remain.
5. **Roster order:** the first eligible member after the deceased leader wins, independent of rank and entity allocation.
6. **Cross-side/missing components:** candidate rejected without corrupting forest.
7. **Simultaneous casualties:** leader and first successor die; next living candidate assumes command.
8. **Plan inheritance:** successor gets same plan/expiry, fresh progress, and no direct concrete order.
9. **Redelegation:** Hold Line stations are regenerated for survivors and newer task revisions supersede old ones.
10. **Comms failure:** successor assumes command locally but subordinate directives travel only when the packet network permits.
11. **Player root death:** mission resolves as Defeat and `PlayerControlledUnit` does not transfer.
12. **Mission teardown:** no succession state/messages leak into the next mission.

## Acceptance criteria

- No dead unit remains in `CommandForest` after the Cleanup pass.
- Every successful rewrite preserves forest coherence and cannot create a cycle or cross-side edge.
- Successor selection is deterministic across runs.
- Active plan intent survives coordinator death, while stale delegation progress does not.
- Affected living units receive revised directives through comms; succession does not directly install movement/combat orders.
- Player-command death always resolves as Defeat and control never transfers.
- Succession is visible in traces/diagnostics without leaking hidden enemy truth.
- `cargo test`, `cargo clippy --all-targets`, and `cargo fmt --check` pass.

## Explicitly deferred

- Delayed/contested knowledge of a commander's death.
- Elections, competing claimants, morale, refusal, and rank disputes.
- Temporary command loss caused only by comms isolation.
- Squad roles, fireteams, staff billets, and nested organizational entities.
- Reinforcement spawning and campaign-level replacement officers.
