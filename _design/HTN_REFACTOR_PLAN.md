# HTN Executor Refactor Plan

Goal: make the HTN module cheap to extend before info-packet integration. Six phases,
each independently compilable and testable. **Complete phases in order. Run
`cargo test` and `cargo clippy` after every phase. Do not start a phase until the
previous one is green.** No behavior changes are intended except where explicitly
noted; existing tests are the regression harness — update them only as instructed.

Files involved:

- `src/ai/htn/{mod,domain,planner,soldier,state,synthesis,executor,trace}.rs`
- `src/gameplay/simulation.rs` (`UnitOrder`, `move_units`)
- `src/gameplay/combat/components.rs` (`CombatOrder`), `src/gameplay/combat/resolution.rs`
- `src/gameplay/lifecycle.rs`
- `src/player/selection.rs`
- `src/screens/mission.rs` (soldier spawn, ~line 782 and ~849)
- `docs/ai/htn.md`, new `docs/gameplay/orders.md`

---

## Phase 0 — Baseline

1. Run `cargo test`. Record the passing test list. All later phases must keep these
   passing (modulo instructed test edits).

---

## Phase 1 — Order provenance

**Problem:** `executor.rs::has_external_order` *infers* whether an order came from the
player by diffing current orders against `PlanRunner.issued_orders`, with a
special-case carve-out for `CombatOrder::HoldFire`. Replace inference with explicit
provenance tags. This deletes `IssuedOrders` entirely.

### 1.1 New module `src/gameplay/orders.rs`

`UnitOrder` and `CombatOrder` remain separate components (they are orthogonal —
a unit can be moving *and* firing — and are read by different system sets).
Only the provenance side is unified, via a marker-typed generic component so
arbitration helpers work over any order kind without duplication.

```rust
#![doc = include_str!("../../docs/gameplay/orders.md")]

use bevy::prelude::*;
use std::marker::PhantomData;

/// Who issued an order. Provenance decides arbitration: HTN planning yields to
/// Player orders; Doctrine marks default postures that never suppress planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSource {
    /// Direct player directive. Preempts autonomous planning.
    Player,
    /// Issued by the unit's own HTN executor.
    Htn,
    /// Default posture (e.g. spawn-time HoldFire, combat-resolution decay).
    /// Never treated as a directive.
    Doctrine,
    // Future: Superior(Entity) — delegated order received via comms.
}

/// Provenance of an order component of type `O`.
///
/// INVARIANT: `OrderProvenance::<O>` is present if and only if `O` is present
/// on the same entity. Every site that inserts/removes/overwrites the order
/// component must do the same to its provenance in the same command batch.
///
/// The `PhantomData<O>` marker exists purely so `OrderProvenance<UnitOrder>`
/// and `OrderProvenance<CombatOrder>` are distinct component types (ECS keys
/// components by concrete type) while sharing all logic.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderProvenance<O: Component> {
    pub source: OrderSource,
    _marker: PhantomData<O>,
}

impl<O: Component> OrderProvenance<O> {
    pub const fn new(source: OrderSource) -> Self {
        Self { source, _marker: PhantomData }
    }

    pub const fn player() -> Self { Self::new(OrderSource::Player) }
    pub const fn htn() -> Self { Self::new(OrderSource::Htn) }
    pub const fn doctrine() -> Self { Self::new(OrderSource::Doctrine) }
}

/// True if the order (if present) came from the player and must preempt HTN planning.
pub fn is_player_sourced<O: Component>(src: Option<&OrderProvenance<O>>) -> bool {
    src.is_some_and(|s| s.source == OrderSource::Player)
}

/// Remove an HTN-sourced order and its provenance. No-op for other sources.
pub fn clear_if_htn<O: Component + bevy::prelude::Bundle>(
    commands: &mut Commands,
    entity: Entity,
    src: Option<&OrderProvenance<O>>,
) {
    if src.is_some_and(|s| s.source == OrderSource::Htn) {
        commands.entity(entity).remove::<(O, OrderProvenance<O>)>();
    }
}
```

Note: `O` needs `Component + Bundle` for the tuple-remove above; both `UnitOrder`
and `CombatOrder` already satisfy this (any `Component` auto-implements `Bundle`
for tuple removal, but pin the bound explicitly if the compiler complains).

Type aliases for ergonomics — put these in `orders.rs` after the imports of the
order types (or in the consuming modules):

```rust
pub type UnitOrderSource = OrderProvenance<crate::gameplay::simulation::UnitOrder>;
pub type CombatOrderSource = OrderProvenance<crate::gameplay::combat::CombatOrder>;
```

The rest of the plan refers to `UnitOrderSource` / `CombatOrderSource` — those
names now resolve to these aliases, so the touch-site instructions below are
unchanged in spelling.

Create `docs/gameplay/orders.md` (short: what provenance is for, the invariant, the
arbitration rule "external ⟺ source is Player"). Register the module in
`src/gameplay/mod.rs` (follow the pattern of the sibling modules; no plugin needed —
this module is components only).

### 1.2 Tag every insertion site

- `src/player/selection.rs` (~lines 162 & 166): wherever `CombatOrder::FireAt` /
  `UnitOrder::MoveTo` are inserted, insert `CombatOrderSource::player()` /
  `UnitOrderSource::player()` in the same `.insert((...))` tuple.
- `src/screens/mission.rs` (~line 849): the spawn-time `CombatOrder::HoldFire` gets
  `CombatOrderSource::doctrine()` added to the spawn bundle.
- `src/gameplay/debug_powers.rs`: check for any order insertions (`rg "insert.*Order"`);
  tag them with `::player()`.

### 1.3 Maintain the invariant at every removal/mutation site

- `src/gameplay/simulation.rs::move_units`: both `remove::<UnitOrder>()` calls also
  `.remove::<UnitOrderSource>()`.
- `src/gameplay/lifecycle.rs` (~lines 32–33): add `.remove::<UnitOrderSource>()` and
  `.remove::<CombatOrderSource>()` next to the order removals.
- `src/gameplay/combat/resolution.rs` (~line 72): where `*order = CombatOrder::HoldFire`
  decays a FireAt, the source must become Doctrine. Add
  `Option<&mut CombatOrderSource>` to that system's query; when decaying, if the
  option is `Some(mut src)` set `*src = CombatOrderSource::doctrine()`. (Option,
  not required, so existing resolution tests that spawn bare `CombatOrder` keep
  passing.)

### 1.4 Rewrite executor arbitration

In `src/ai/htn/executor.rs`:

1. Delete `IssuedOrders` struct and the `issued_orders` field of `PlanRunner`.
2. Replace `has_external_order` with a call to the generic helper:

```rust
fn has_external_order(
    unit_source: Option<&UnitOrderSource>,
    combat_source: Option<&CombatOrderSource>,
) -> bool {
    is_player_sourced(unit_source) || is_player_sourced(combat_source)
}
```

3. Delete `clear_htn_orders_if_unchanged` entirely. Its replacement is two calls
   to the generic `clear_if_htn` helper at each former call site:

```rust
clear_if_htn::<UnitOrder>(&mut commands, entity, unit_source);
clear_if_htn::<CombatOrder>(&mut commands, entity, combat_source);
```

4. In the three executor systems, add `Option<&UnitOrderSource>` and
   `Option<&CombatOrderSource>` to the queries and thread them into the helpers
   above. Delete all `issued_orders` bookkeeping (assignments in
   `start_pending_steps`, resets in `advance_plan_execution`, the field in every
   `PlanRunner` literal).
5. In `start_pending_steps`, wherever HTN orders are inserted
   (`UnitOrder::Hold`/`MoveTo`, `CombatOrder::FireAt`/`HoldFire`), insert the
   matching provenance in the same tuple, e.g.
   `(UnitOrder::MoveTo { .. }, UnitOrderSource::htn())`.
6. The HoldFire regression comment/carve-out in the old `has_external_order` is now
   obsolete — Doctrine provenance handles it. Delete the carve-out.

### 1.5 Tests

- Update executor tests: remove `issued_orders` from all `PlanRunner` literals; where
  tests previously relied on issued-order diffing (e.g.
  `player_unit_order_replaces_htn_order_without_being_cleared`), insert the
  appropriate `*OrderSource` components (`Player` for the player order,
  `Htn` where the test simulated HTN-issued orders).
- `deliberation_creates_runner_despite_default_hold_fire_posture`: give the spawned
  HoldFire a `CombatOrderSource::doctrine()`. The test's assertion is unchanged
  and now verifies the provenance rule.
- Add one new test: entity with `UnitOrder::MoveTo` + `UnitOrderSource(Player)` and a
  running `PlanRunner` → after `deliberate_autonomous_units`, `PlanRunner` removed,
  order and source still present.
- Add one new test: `clear_htn_orders` removes Htn-sourced orders but leaves
  Player-sourced ones untouched (can be a direct unit test on a `World`).

---

## Phase 2 — Belief synthesis as a system

**Problem:** `synthesize_planner_state(8 args)` is called inside all three executor
systems, each dragging a near-identical 10-tuple query.

### 2.1 New component in `src/ai/htn/synthesis.rs`

```rust
/// Per-unit planning snapshot, refreshed once per tick before Thinking systems run.
/// This is the unit's belief state — the single input to deliberation, step
/// dispatch, and step polling. Debug/trace views should read this, not raw memory.
#[derive(Component, Debug, Clone, Default)]
pub struct PlannerBelief {
    pub state: PlannerState,
}
```

### 2.2 New system in `synthesis.rs`

```rust
pub fn synthesize_beliefs(
    clock: Res<SimulationClock>,
    recent_shots: Res<RecentResolvedShots>,
    mut units: Query<
        (
            &BattlefieldPosition, &Health, &Inventory, &PerceptionMemory,
            Option<&AuditorySensor>, Option<&UnitOrder>, &mut PlannerBelief,
        ),
        (With<Soldier>, With<Alive>, With<Autonomous>),
    >,
) {
    for (position, health, inventory, memory, auditory, order, mut belief) in &mut units {
        belief.state = synthesize_planner_state(
            &clock, position, health, inventory, memory, auditory, order,
            &recent_shots.shots,
        );
    }
}
```

Note: `RecentResolvedShots` and `Autonomous` live in `executor.rs`; import them.
(Moving `RecentResolvedShots` + `collect_recent_resolved_shots` into `synthesis.rs`
is optional; if done, fix imports in executor tests.)

### 2.3 Wire up

- In `HtnExecutorPlugin`, change the Thinking chain to:
  `(synthesize_beliefs, advance_plan_execution, deliberate_autonomous_units, start_pending_steps).chain()`.
- In `src/screens/mission.rs` (~line 782) add `PlannerBelief::default()` to the tuple
  inserted alongside `Autonomous` and `DecisionTrace::default()`.

### 2.4 Slim the executor systems

In all three executor systems: remove `BattlefieldPosition`, `Health`, `Inventory`,
`PerceptionMemory`, `AuditorySensor` from the queries and remove the
`synthesize_planner_state` calls; add `&PlannerBelief` and use `&belief.state`
wherever `state` was used. `deliberate_autonomous_units` computes the digest from
`&belief.state`. Remove now-unused `Res` params (`clock`, `recent_shots`) from
systems that no longer need them.

### 2.5 Tests

Executor tests must now (a) spawn `PlannerBelief::default()` on units and (b) add
`synthesize_beliefs` before the system under test, e.g.
`app.add_systems(Update, (synthesize_beliefs, deliberate_autonomous_units).chain())`.
Keep assertions unchanged. Tests that relied on `PerceptionMemory` contents (e.g.
`equal_mtr_candidate_with_different_bound_operator_is_adopted`) work unchanged once
synthesis runs in the chain.

---

## Phase 3 — Collocate operator behavior

**Problem:** adding a `BoundOperator` variant touches six scattered sites. Make it a
one-file change.

### 3.1 New file `src/ai/htn/operators.rs`

Move from `domain.rs`: the `BoundOperator` enum and its `describe()` impl.
Move from `executor.rs`: `StepPoll`, `poll_move`, `poll_fire`,
`MOVE_DESTINATION_EPSILON_M`, `FRESH_HOSTILE_TICKS`.

Add two methods so dispatch/poll live next to the enum:

```rust
impl BoundOperator {
    /// Issue the orders that realize this operator. All orders are tagged
    /// `OrderSource::Htn`.
    pub fn dispatch(&self, commands: &mut Commands, entity: Entity) {
        match *self {
            BoundOperator::Hold => {
                commands.entity(entity).insert((
                    UnitOrder::Hold, UnitOrderSource::htn(),
                    CombatOrder::HoldFire, CombatOrderSource::htn(),
                ));
            }
            BoundOperator::MoveTo { destination_m } => {
                commands.entity(entity).insert((
                    UnitOrder::MoveTo { destination_m },
                    UnitOrderSource::htn(),
                ));
            }
            BoundOperator::FireAt { target } => {
                commands.entity(entity).insert((
                    CombatOrder::FireAt { target },
                    CombatOrderSource::htn(),
                ));
            }
        }
    }

    /// Check whether the running step finished, failed, or continues.
    pub fn poll(
        &self,
        state: &PlannerState,
        unit_order: Option<&UnitOrder>,
        combat_order: Option<&CombatOrder>,
    ) -> StepPoll {
        match *self {
            BoundOperator::Hold => StepPoll::Running,
            BoundOperator::MoveTo { destination_m } => poll_move(destination_m, unit_order),
            BoundOperator::FireAt { target } => poll_fire(target, combat_order, state),
        }
    }
}
```

`poll_move`/`poll_fire` become private helpers in this file, bodies unchanged.

### 3.2 Rewire

- `mod.rs`: add `pub mod operators;`.
- `domain.rs`: `pub use super::operators::BoundOperator;` so all existing imports keep
  working. The `bind_*` helper fns stay in `domain.rs`.
- `executor.rs::start_pending_steps`: replace the operator `match` with
  `step.operator.dispatch(&mut commands, entity);`.
- `executor.rs::advance_plan_execution`: replace the outcome `match` with
  `step.operator.poll(&belief.state, current_order, combat_order)`.

### 3.3 Tests

Move any tests that directly exercised `poll_move`/`poll_fire` (if none exist inline,
add two small ones in `operators.rs`: move-succeeds-when-order-removed,
fire-succeeds-when-out-of-ammo). Existing executor tests are the integration
coverage; they must pass unchanged.

---

## Phase 4 — Digest moves next to state

**Problem:** `PlannerStateDigest` is the replan-relevance contract for
`PlannerState`, but lives in `executor.rs` where it silently drifts.

1. Move `PlannerStateDigest`, `PlannerStateDigest::from_state`, and `health_band`
   from `executor.rs` into `src/ai/htn/state.rs`, directly below `PlannerState`.
2. `FRESH_HOSTILE_TICKS` now lives in `operators.rs` (Phase 3); import it, or
   reference `soldier::FRESH_CONTACT_TICKS` directly — pick one, delete the alias.
3. Add this doc comment on the struct:

```rust
/// Quantized projection of `PlannerState` used to detect decision-relevant change.
/// The executor replans only when this digest changes.
///
/// CONTRACT: when adding a field to `PlannerState`, decide HERE whether it is
/// decision-relevant. If any domain precondition reads the new field, the digest
/// must reflect it (band/bool-quantized, never raw floats or ticks), or units will
/// silently fail to replan when it changes.
```

4. `executor.rs` imports the digest from `state`. Test literals keep working via the
   new path (fix imports only).

---

## Phase 5 — Domain registry

**Problem:** `HtnDomainRegistry { soldier: Option<Domain> }` hardcodes one archetype.

1. In `executor.rs` (or a new tiny `registry.rs` if preferred):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DomainId {
    Soldier,
}

/// Which HTN domain this autonomous unit plans with.
#[derive(Component, Debug, Clone, Copy)]
pub struct DomainRef(pub DomainId);

#[derive(Resource, Default)]
pub struct HtnDomainRegistry {
    pub domains: HashMap<DomainId, Domain>,
}
```

2. `mod.rs` `HtnPlugin`: build the registry with
   `domains: HashMap::from([(DomainId::Soldier, soldier::build_soldier_domain())])`.
3. `deliberate_autonomous_units`: add `&DomainRef` to the query; replace
   `registry.soldier.as_ref()` (which hoisted the lookup outside the loop) with a
   per-unit `registry.domains.get(&domain_ref.0)`; `continue` with a `warn!` if
   missing.
4. `src/screens/mission.rs` (~line 782): add `DomainRef(DomainId::Soldier)` to the
   autonomous-unit insert tuple.
5. Tests: registry literals become
   `HtnDomainRegistry { domains: HashMap::from([(DomainId::Soldier, test_domain())]) }`;
   spawned test units get `DomainRef(DomainId::Soldier)`.

---

## Phase 6 — Extension recipe doc

Rewrite `docs/ai/htn.md` (currently 16 lines) with:

1. **Architecture overview** (~half page): domain (tasks/methods, method order =
   priority), planner (decomposition + MTR), synthesis (`PlannerBelief`, once per
   tick), executor (deliberate → dispatch → poll), provenance-based arbitration,
   trace. One sentence each, naming the file that owns it.
2. **Recipe: add an operator** — checklist: variant in `operators.rs`; `describe`;
   `dispatch` (orders + `OrderSource::Htn` tags); `poll`; a `bind_*` helper in
   `domain.rs`; a test.
3. **Recipe: add a `PlannerState` field** — checklist: field + default in `state.rs`;
   populate in `synthesize_planner_state`; **decide digest relevance in
   `PlannerStateDigest` (contract comment)**; use in conditions.
4. **Recipe: add a task/method** — checklist: condition fns, `primitive_with_reason`
   (reason string = doctrine justification, surfaces in trace), method insertion
   position = priority, planner-effects note (effects simulate belief change during
   decomposition only — see `no_fire_planner_effect` for the pattern of *not*
   simulating stochastic outcomes), tests asserting expected MTR.
5. **Recipe: add a domain** — `DomainId` variant, builder fn, register in `HtnPlugin`,
   `DomainRef` at spawn.

---

## Final verification

1. `cargo test` — full suite green.
2. `cargo clippy -- -D warnings` — clean.
3. `rg "IssuedOrders|issued_orders" src/` — zero hits.
4. `rg "registry.soldier" src/` — zero hits.
5. Manual smoke (if a display is available): `cargo run`, start a mission, verify
   autonomous units still engage/investigate/retreat, and a player move order issued
   to an autonomous unit preempts its plan and is not cleared on plan teardown.
