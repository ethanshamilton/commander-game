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

For a dead commander, candidates are its **living direct subordinates**. A candidate must:

- have `Soldier + Alive`;
- have the same `Side` as the dead commander;
- still be a direct child in the pre-transition forest.

Choose by:

1. authored succession priority (lower number wins);
2. higher military rank;
3. stable `UnitId` lexical order;
4. Bevy entity bits only as a final fallback for malformed runtime-spawned units.

Add a mission-authored succession value to `MissionUnit` and a runtime component:

```rust
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SuccessionPriority(pub u16);
```

Do not infer the whole policy from rank. Two privates need a stable doctrinal order, and entity creation order is not a good game rule.

Add total rank ordering (`Rank::command_precedence`) rather than relying on enum declaration order.

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

Do **not** copy the predecessor's delegation progress. The successor recomputes against its new direct members.

Existing subordinate `AssignedTask`s remain valid until superseded, cancelled, or expired. On the next planning pass, the successor emits revised tasks to all affected living direct members. Newer assignment stamps supersede the predecessor's tasks.

### Player command node

If `D` has `PlayerControlledUnit`, move that component to `S`. This keeps the player an in-world command node and preserves the `single()` invariant used by selection/order systems.

Also:

- change `SelectedUnit` from `D` to `S` if the dead controlled unit was selected;
- keep dead-unit knowledge/history intact;
- show a HUD notification naming the successor;
- if no successor exists, clear `PlayerControlledUnit`, disable command actions, and define mission defeat via an explicit objective if desired rather than crashing queries.

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

### Change message

Emit after a successful atomic rewrite:

```rust
#[derive(Message)]
pub struct CommandStructureChanged {
    pub tick: u64,
    pub reason: CommandStructureChangeReason,
    pub affected_roots: Vec<Entity>,
}
```

The plan system consumes it and invalidates execution progress for affected coordinators. Avoid polling or hashing the entire forest every tick.

### Delegation signature

Store the exact inputs that determine decomposition:

```rust
pub struct DelegationSignature {
    pub plan_id: CommandPlanId,
    pub assignment_revision: u64,
    pub coordinator: Entity,
    pub participants: Vec<UnitId>,
}
```

Participants are sorted and contain only living direct members plus the coordinator. If the signature changes:

1. increment the local delegation revision;
2. clear the set of accepted subordinate assignments;
3. recompute all stations/routes;
4. send revised tasks to **every** living participant whose directive changed;
5. explicitly cancel obsolete tasks where the recipient is still alive; and
6. update the coordinator's own target.

This fixes more than leader death: subordinate casualties, reinforcement, reparenting, and reassignment all use the same path.

### In-flight packets

Old packets may arrive after succession. Required recipient checks:

- origin had authority for the recipient under the relevant command epoch;
- assignment stamp is newer than the installed one;
- plan/task is not cancelled or expired.

For V1, the current forest plus strict newer revision is sufficient because succession occurs between ticks and the successor reissues on the next tick. If tests expose legitimate predecessor packets being rejected after topology changes, add a short-lived `CommandEpochHistory` rather than weakening authority globally.

A packet from the dead predecessor that was not accepted before restructuring should normally be consumed and rejected. The successor's new directive is authoritative.

## Succession processing system

Create `src/gameplay/command_succession.rs` and `CommandSuccessionPlugin`.

Per `UnitDied`:

1. Snapshot dead unit's side, parent, direct children, assigned plan, and control marker.
2. Filter eligible candidates using `Alive`, side, priority, rank, and `UnitIdentity`.
3. Choose deterministically with a pure `choose_successor` function.
4. Call one atomic `CommandForest::succeed` mutation.
5. Remove command-only components from the deceased (`AssignedCommandPlan`, pending delegation, progress, assumed-command marker).
6. If a successor exists:
   - install inherited `AssignedCommandPlan` if applicable;
   - reset/generate delegation progress;
   - add `AssumedCommand`;
   - transfer `PlayerControlledUnit` if needed.
7. Emit `CommandSucceeded` and `CommandStructureChanged` messages.
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

### C1 — Lifecycle event and idempotent death

- Add `UnitDied { entity, tick, cause }`.
- Make combat and debug kill use one transition API.
- Return early if already dead/not alive.
- Add lifecycle tests proving one event and capability stripping.

### C2 — Deterministic policy data

- Add `SuccessionPriority` to mission authoring and soldier spawn data.
- Add explicit rank precedence.
- Validate duplicate/missing succession priorities among siblings at mission instantiation; duplicates may fall back deterministically but should warn.
- Implement and unit-test pure candidate ordering.

### C3 — Atomic forest succession

- Implement `CommandForest::succeed` and invariant validation.
- Cover root, middle node, leaf, no successor, malformed successor, and cycle safety.
- Make authority call sites life-aware.

### C4 — Runtime succession

- Add plugin/system in `SimulationSet::Cleanup`.
- Transfer player control and selected unit.
- Emit structure/succession messages.
- Handle multiple deaths deterministically.

**Done when:** debug-killing a squad leader promotes the authored successor before the next simulation tick and all surviving squad members have the new direct superior.

### C5 — Plan assumption

- Transfer active plan intent without copying stale execution progress.
- Add `AssumedCommand` and trace/UI status.
- Preserve expiry/fallback data.
- Ensure a successor with no inherited plan falls back to ordinary infantry behavior.

### C6 — Dynamic redelegation

- Replace `delegated_to`-only invalidation with a delegation signature/revision.
- Recompute and resend changed directives after any command-membership change.
- Add cancellation for obsolete live recipients.
- Verify concrete orders remain HTN-sourced.

### C7 — Hardening

- Simultaneous leader/successor death.
- Root commander death and player-control transfer.
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
5. **Priority:** authored priority beats rank; rank beats stable ID when priority ties.
6. **Cross-side/missing components:** candidate rejected without corrupting forest.
7. **Simultaneous casualties:** leader and first successor die; next living candidate assumes command.
8. **Plan inheritance:** successor gets same plan/expiry, fresh progress, and no direct concrete order.
9. **Redelegation:** Hold Line stations are regenerated for survivors and newer task revisions supersede old ones.
10. **Comms failure:** successor assumes command locally but subordinate directives travel only when the packet network permits.
11. **Player root death:** exactly one living `PlayerControlledUnit` remains when a successor exists.
12. **Mission teardown:** no succession state/messages leak into the next mission.

## Acceptance criteria

- No dead unit remains in `CommandForest` after the Cleanup pass.
- Every successful rewrite preserves forest coherence and cannot create a cycle or cross-side edge.
- Successor selection is deterministic across runs.
- Active plan intent survives coordinator death, while stale delegation progress does not.
- Affected living units receive revised directives through comms; succession does not directly install movement/combat orders.
- Player control transfers safely or degrades to a defined no-commander state.
- Succession is visible in traces/diagnostics without leaking hidden enemy truth.
- `cargo test`, `cargo clippy --all-targets`, and `cargo fmt --check` pass.

## Explicitly deferred

- Delayed/contested knowledge of a commander's death.
- Elections, competing claimants, morale, refusal, and rank disputes.
- Temporary command loss caused only by comms isolation.
- Full squad/formation entities and staff roles.
- Reinforcement spawning and campaign-level replacement officers.
