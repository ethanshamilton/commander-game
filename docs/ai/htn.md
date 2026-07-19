# HTN Planner

The HTN module is the per-unit hierarchical task network planner, plus the ECS
plumbing that turns its output into gameplay orders. It's split into a pure
planning core (no Bevy `Commands`, no side effects) and thin ECS adapters
around it, so the decomposition logic is unit-testable without spinning up an
`App`.

## 1. Architecture overview

- **`domain.rs`** — `Task`/`Method`/`Domain`/`DomainBuilder`. A domain is a
  tree: compound tasks decompose into ordered `Method`s (lower index = higher
  priority; the first method whose precondition passes wins), bottoming out in
  primitive tasks that bind to a `BoundOperator` at plan time. `bind_*` helper
  fns (e.g. `bind_fire_at_nearest_hostile`) live here — they're the glue
  between doctrine-level task names and concrete parameters.
- **`soldier.rs` / `leader.rs`** — composable task installers and the infantry
  domain builder. Every infantry unit has the same behavioral repertoire;
  leadership methods become applicable only when planner state contains current
  command responsibility and an assigned mission. Leadership is therefore a
  transient role rather than a rank-selected domain.
- **`operators.rs`** — `BoundOperator` (the concrete, parameterized action a
  primitive task resolves to: `Hold`/`MoveTo`/`FireAt`/delegation) plus its `dispatch()`
  (turn the operator into `MovementOrder`/`CombatOrder` insertions, tagged
  `OrderSource::Htn`) and `poll()` (has the running step finished, failed, or
  is it still in flight). This is the one file that knows how operators talk
  to gameplay.
- **`state.rs`** — `PlannerState`, the decision-oriented belief snapshot the
  planner actually reasons over (not ground truth — see below), and
  `PlannerStateDigest`, its quantized projection used to gate replanning.
- **`synthesis.rs`** — `PlannerBelief` component + `synthesize_beliefs`
  system. Runs once per tick, before any other Thinking system, converting
  unit-local simulation data (`PerceptionMemory`, `Health`, `Inventory`,
  current `MovementOrder`, recent resolved shots) into each unit's
  `PlannerBelief.state`. Every other Thinking system reads `&PlannerBelief`
  instead of re-deriving state from raw components.
- **`planner.rs`** — the pure decomposition algorithm: recursive DFS over the
  domain tree, simulating primitive effects on a scratch `PlannerState` as it
  descends, backtracking on precondition/binding failure, producing a `Plan`
  (ordered `BoundStep`s) tagged with an `Mtr` (method traversal record — the
  sequence of method indices chosen at each compound task, compared
  lexicographically so shallower higher-priority choices always outrank
  deeper ones).
- **`executor.rs`** — the ECS adapter. Three systems, chained every
  `FixedUpdate` tick in `SimulationSet::Thinking`:
  `deliberate_autonomous_units` (decide whether to keep, replace, or create a
  `PlanRunner`, MTR-gated), `start_pending_steps` (dispatch the current step's
  operator), `advance_plan_execution` (poll the running step, advance or tear
  down the runner). Also owns `HtnDomainRegistry` (`DomainId -> Domain` map)
  and `DomainRef` (which domain a unit plans with).
- **Expiry fallback** — the newest expired mission/task projects its assigned
  fallback `PositionTarget` into planner state. The infantry root prioritizes
  fallback below survival and fresh-contact engagement but above normal
  assignments, investigation, and idle behavior. Units move to their formation
  station, adopt its heading, and hold until superseded.
- **Arbitration** — provenance-based, not inference-based. See
  `docs/gameplay/orders.md`: a player-sourced order is external for its own
  order lane. Player movement blocks autonomous move/hold writes, but does not
  block autonomous firing; player combat orders similarly block autonomous
  combat writes. Htn- and Doctrine-sourced orders never suppress planning.
- **`trace.rs`** — `DecisionTrace`/`TraceEvent`, a bounded ring buffer per unit
  recording plan creation/rejection/replan/step events, for debugging *why* a
  unit did what it did. Executor writes are edge-triggered: consecutive
  semantically identical events are suppressed without marking the component
  changed, while the same event after an intervening transition is retained.

`PlannerState` is not ground-truth world state — it's what the unit currently
believes, synthesized from its own memory/components. The planner must never
query enemy ground truth directly; if a domain needs new information, it goes
through `synthesize_planner_state` into `PlannerState`, not around it.

## 2. Recipe: add an operator

New primitive action a unit can be commanded to do (a new `BoundOperator`
variant):

1. Add the variant to `BoundOperator` in `operators.rs`.
2. Extend `describe()` (shows up in `DecisionTrace`).
3. Extend `dispatch()` — insert the gameplay order component(s) it maps to,
   each paired with `MovementOrderSource::htn()` / `CombatOrderSource::htn()` in
   the same tuple (see the invariant in `docs/gameplay/orders.md`: order and
   provenance are inserted/removed together, always).
4. Extend `poll()` — decide `Running`/`Succeeded`/`Failed(&'static str)` from
   current belief state and/or the current order component. `Succeeded` and
   `Failed` are both terminal (the runner clears HTN orders and advances or
   tears down either way); they only differ in the trace event recorded.
5. Add a `bind_*` helper in `domain.rs` that binds a primitive task's
   preconditions to this operator's parameters.
6. Add a test in `operators.rs` covering the new `poll()` branch(es).

## 3. Recipe: add a `PlannerState` field

New belief the planner needs to reason about:

1. Add the field (with a sane default) to `PlannerState` in `state.rs`.
2. Populate it in `synthesize_planner_state` (`synthesis.rs`) from whatever
   raw components/resources carry the ground truth this belief derives from.
3. **Decide digest relevance in `PlannerStateDigest` (see the CONTRACT comment
   on the struct in `state.rs`).** If any domain precondition reads the new
   field, the digest must reflect it — quantized to a band or bool, never a
   raw float or tick count, since the digest only needs to detect
   decision-relevant *change*, not exact values. Skipping this step means
   units silently fail to replan when the field changes.
4. Use the field in a condition fn (`domain.rs`/`soldier.rs`) or a `bind_*`
   helper.

## 4. Recipe: add a task/method

New doctrine — a new thing a unit can decide to do, or a new condition under
which it prefers an existing action:

1. Write condition fns (`fn(&PlannerState) -> bool`) for the method's
   preconditions.
2. Register the primitive with `primitive_with_reason` — the reason string is
   the doctrine-level justification ("health below 35% with hostile contact;
   survival outranks engagement"), and it surfaces directly in
   `DecisionTrace::StepStarted`. Write it for a human debugging the trace, not
   for the compiler.
3. Insert the `Method` at the priority position that reflects its urgency —
   methods are tried in list order, first passing precondition wins, so
   position *is* priority. See `soldier.rs`'s `BeSoldier` root: Survive >
   Engage > Investigate > Idle.
4. If the primitive's binding function affects belief in a way later subtasks
   in the *same* decomposition should see (e.g. a move updates simulated
   `position_m`), give it a real effect fn. If the outcome is stochastic and
   resolved by gameplay systems (e.g. firing — hit/miss/kill is combat
   resolution's job, not the planner's), use a no-op effect and say so in a
   comment — see `no_fire_planner_effect` for the pattern of deliberately
   *not* simulating an outcome the planner has no business predicting.
5. Add a test asserting the expected `Mtr` and bound operator for the
   condition (see `soldier.rs`'s tests for the shape: build a `PlannerState`,
   call `plan()`, assert `plan.mtr` and `plan.steps[0]`).

## 5. Recipe: add a domain

New unit archetype that needs its own doctrine tree (not just a new task in
the existing soldier domain):

1. Add a variant to `DomainId` in `executor.rs`.
2. Write a builder fn (mirror `soldier::build_soldier_domain`) in a new module
   alongside `soldier.rs`.
3. Register it in `HtnPlugin::build` (`mod.rs`):
   `domains: HashMap::from([(DomainId::Soldier, ...), (DomainId::YourNew, ...)])`.
4. Give spawned units of that archetype `DomainRef(DomainId::YourNew)` instead
   of `DomainRef(DomainId::Soldier)` (see `spawn_soldier_at` in
   `screens/scenario.rs` for the current spawn-time wiring pattern —
   `Autonomous` + `DecisionTrace::default()` + `PlannerBelief::default()` +
   `DomainRef`). All soldiers are currently spawned autonomous, including
   player-side units, so direct player orders and HTN orders can be tested
   together.
