# HTN Core — Implementation Plan (single-unit)

Scope: the per-individual planner only. No comms integration, no orders-as-goals,
no intent pointers, no multi-agent coordination, no societies. Those layer on
later (see `AI_COGNITION.md`); this milestone is the planning machinery for one
soldier deciding over its own cognitive environment.

## How it fits the existing architecture

The pieces already in place map cleanly onto HTN roles:

| HTN concept        | Existing code                                             |
|--------------------|-----------------------------------------------------------|
| Sensors            | `ai/perception.rs` → `PerceptionMemory`                   |
| World state source | `PerceptionMemory`, `Health`, `Inventory`, position, etc. |
| Operators          | `UnitOrder::{MoveTo, Hold}`, `CombatOrder::{FireAt, HoldFire}` |
| Execution substrate| `move_units`, `resolve_combat` in the fixed-tick sim      |

The planner is a new layer that reads unit-local memory and writes effector
components. It never touches ground truth (the truth-pipeline boundary), and it
never executes actions itself — it only emits the same order components the
player path uses.

Data flow per tick:

```text
PerceptionMemory + self components
    -> [synthesize] WorldState snapshot
    -> [planner, event-driven] Plan
    -> [executor, every tick] UnitOrder / CombatOrder components
    -> existing movement/combat systems
```

## Module layout

```text
src/ai/htn/
  mod.rs         HtnPlugin, plumbing
  world_state.rs WorldState struct + synthesis from ECS
  domain.rs      Task/Method/Domain types, DomainBuilder
  planner.rs     DFS decomposition, MTR — pure, no ECS deps
  executor.rs    PlanRunner component, step lifecycle, operator dispatch
  trace.rs       DecisionTrace events + per-unit ring buffer
  soldier.rs     the v1 soldier domain definition
```

`planner.rs` and `domain.rs` must stay ECS-free (operate on `WorldState` only)
so they're unit-testable without spinning up an `App`.

## Core types (v1 decisions)

### WorldState

A plain compact struct, **not** a key-value map. Cheap to `Clone` (the planner
copies it to simulate effects), explicit fields, compiler-checked conditions:

```rust
pub struct WorldState {
    pub position_m: Vec2,
    pub health_frac: f32,
    pub has_ammo: bool,
    pub nearest_hostile: Option<HostileSnapshot>, // pos, entity, confidence, staleness
    pub under_fire: bool,                          // derived, v1 can approximate
    pub has_move_target: bool,
    pub tick: u64,
}
```

Synthesized fresh from components each deliberation pass. Map-based/data-driven
world state is a later refactor if/when domains become data (societies); don't
pay that cost now.

### Domain

Static structure, built once at startup, shared by all soldiers (a `Resource`):

- `PrimitiveTask { name, operator: OperatorSpec, preconditions: fn(&WorldState) -> bool, effects: fn(&mut WorldState) }`
- `CompoundTask { name, methods: Vec<Method> }`
- `Method { preconditions: fn(&WorldState) -> bool, subtasks: Vec<TaskId> }`
- `Domain { tasks: Vec<Task>, root: TaskId }`

Conditions/effects as plain `fn` pointers for v1 — fast, simple, testable.
**Method order is priority order** (this is where doctrine and, later, society
parameterization lives — keep methods ordered deliberately, document each).

Operator parameters (e.g. destination for MoveTo) are resolved from the world
state at *plan time* and stored bound into the plan step. v1 has no general
unification — each operator spec knows how to extract its params from
`WorldState` (e.g. `MoveAwayFromNearestHostile` computes its own destination).

### Planner

Straight from Game AI Pro ch. 12: iterative DFS with a decomposition stack,
simulated world-state copy, and a **method traversal record (MTR)**. Output:

```rust
pub struct Plan { pub steps: Vec<BoundStep>, pub mtr: Mtr }
```

MTR comparison gates replanning: a replan triggered by a world-state change is
only adopted if its MTR is strictly higher priority than the running plan's.
This is the anti-thrash mechanism and is in scope for v1 (it's cheap and the
perception system updates memory constantly, so we need it immediately).

### Executor

Per-unit component:

```rust
#[derive(Component)]
pub struct PlanRunner {
    pub plan: Plan,
    pub current: usize,
    pub step_state: StepState, // Pending | Running | Succeeded | Failed
}
```

Step lifecycle each tick:

1. If `Pending`: check step precondition against a fresh `WorldState`. Valid →
   dispatch operator (insert `UnitOrder`/`CombatOrder`), mark `Running`.
   Invalid → plan failed, request replan.
2. If `Running`: poll completion. `MoveTo` completion signal already exists —
   `move_units` removes `UnitOrder` on arrival. `FireAt` has **no natural
   terminal state**; v1 gives it an explicit success/abort condition
   ("target dead / contact lost / out of ammo") checked by the executor.
3. On success: advance to next step; on plan exhaustion, request replan.

Replan triggers: no plan · plan completed · step failed · world-state change
(MTR-gated). v1 "relevant change" detection: re-synthesize `WorldState` each
deliberation pass and compare a small digest of decision-relevant fields
(hostile presence, health band, ammo) — no event infrastructure yet.

### Trace (in scope for v1, non-negotiable)

Per the `AI_COGNITION.md` invariant — *no action without a citable reason*:

```rust
pub enum TraceEvent {
    PlanCreated { root, mtr, steps },
    PlanRejected { reason },            // MTR not better, no valid plan
    StepStarted { task, why: &'static str },
    StepFailed { task, failed_condition },
    Replanned { trigger },
}
```

Stored in a per-unit ring buffer component, emitted via `info!` behind a debug
flag for now. This is the debugging tool for everything below, so it goes in
first, not last.

## Scheduling

Add `SimulationSet::Thinking` between `Reports` and `Combat`:

```text
Clock, Orders, Movement, Sensors, Comms, Reports, [Thinking], Combat, Objectives, Cleanup
```

Thinking reads memory updated by this tick's `Sensors`, writes
`UnitOrder`/`CombatOrder`; `Combat` consumes fire orders same-tick, `Movement`
consumes move orders next tick. One-tick reaction latency is acceptable (50ms —
arguably more realistic than instant).

Planning is event-driven (triggers above), execution polling is per-tick and
cheap. Per-frame plan budget / staggering is **out of scope** until unit counts
demand it.

## Coexistence with player control

The planner only drives entities with an `Autonomous` marker component. The
existing player click→`UnitOrder` path is untouched. If the player issues an
order to an autonomous unit, v1 rule: player order wins — executor aborts its
plan and goes dormant until the player order completes (component removed),
then resumes deliberating. Reconciling this properly is the *orders-as-goals*
milestone, not this one.

## The v1 soldier domain (`soldier.rs`)

Deliberately tiny — it exists to validate machinery, not to be smart:

```text
BeSoldier (root compound)
  ├─ Method: Survive        [health_frac < 0.35 && hostile present]
  │    └─ MoveAwayFromNearestHostile
  ├─ Method: Engage         [hostile present && has_ammo && confidence fresh]
  │    ├─ FireAtNearestHostile   (until dead/lost/dry)
  ├─ Method: Investigate    [hostile contact stale but recent]
  │    └─ MoveTo(last known position)
  └─ Method: Idle           [always]
       └─ Hold
```

Four methods, ~5 primitives. Method order encodes the standing-goal stack from
`AI_COGNITION.md` (survive > engage > investigate > idle). `RejoinComms` and
`ExecuteCurrentOrder` slot in later without structural change.

## Milestones

Each lands independently with tests:

- **M1 — pure planner.** `domain.rs` + `planner.rs` + `world_state.rs` (struct
  only). Unit tests: hand-built domains, assert plans + MTR ordering + no valid
  plan cases. Zero ECS.
- **M2 — world-state synthesis.** System building `WorldState` from
  `PerceptionMemory`/`Health`/`Inventory`/position. Headless `App` test:
  spawn soldier + hostile, tick, assert snapshot fields.
- **M3 — executor.** `PlanRunner`, `Deliberation` set, operator dispatch,
  completion detection, replan triggers, trace events. Test: scripted domain
  ("move to X then hold"), assert order components appear/complete and trace
  is coherent.
- **M4 — soldier domain + scenario.** Wire `soldier.rs`, add `Autonomous` to a
  test-mission enemy. Acceptance: soldier idles → hostile appears → engages →
  hostile leaves LOS → investigates last known position → resumes idle; when
  wounded, breaks off and retreats. Verified both by eye and by a headless
  test asserting the trace-event sequence.
- **M5 (stretch) — trace overlay.** Minimal debug UI: select unit, see recent
  trace ring. Purely additive.

## Explicit non-goals (deferred)

Orders as installed goals · intent pointers · comms/packet integration ·
multi-agent anything · data-driven/serialized domains · society parameters ·
utility scoring within methods · plan-time pathfinding awareness · performance
staggering.

## Open questions (fine to resolve during implementation)

- `under_fire` derivation — is there enough info in memory now, or does v1
  approximate it as "fresh hostile contact within N meters"?
- Does `Investigate` need a timeout primitive ("search for T seconds") or is
  arrival-at-last-known-position enough for v1?
- Trace ring buffer size / whether traces should also become `Message`s for
  future eval hooks.
