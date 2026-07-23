# Hybrid Utility + HTN AI System

## Status

Design sketch. Not yet implemented.

## Summary

Replace the infantry HTN root's fixed priority ordering with utility-based goal
selection while retaining HTN decomposition for procedural planning.

The resulting decision pipeline is:

```text
unit-local beliefs
    -> utility-based goal selection
    -> HTN decomposition of the selected goal
    -> plan execution
    -> gameplay orders
    -> world outcomes
    -> updated unit-local beliefs
```

The utility layer answers **what should this unit pursue now?** The HTN answers
**how does this unit pursue that kind of goal?**

This preserves the HTN's legible procedural knowledge while removing the rigid
global ordering currently encoded by the `BeInfantry` root:

```text
Survive > Engage > Fallback > AssignedTask > CommandPlan > Investigate > Idle
```

Under the hybrid model, those alternatives become independently scored goals.
Survival, engagement, and obedience can therefore trade off according to the
unit's believed situation rather than list position alone.

## 1. Motivation

The current HTN performs two conceptually different jobs:

1. It chooses which high-level intention the unit should adopt.
2. It decomposes that intention into executable operators.

HTNs are a strong fit for the second job. Tasks such as executing a Hold Line
plan or occupying an assigned station have authored internal structure that
should remain explicit, testable, and traceable.

The first job is less well served by ordered HTN methods. Method order imposes
lexical priority: if `Survive` applies, no amount of assignment urgency, nearby
support, target vulnerability, or tactical importance can make `Engage` or
`ExecuteAssignedTask` preferable. Adding exceptions requires increasingly
specific methods and preconditions, making doctrine brittle and combinatorial.

Utility selection replaces categorical ordering with graded competition among
reasons. A wounded unit supported by its squad may continue firing, while a
healthier isolated unit facing overwhelming opposition may retreat. The HTN
then supplies the procedural knowledge needed to realize whichever goal wins.

Utility selection is still authored. It moves doctrine from ordered method
positions into:

- feasibility constraints;
- selected considerations;
- response curves and weights;
- switching and commitment policy.

The advantage is not neutrality. It is contextual tradeoff, tunability, and a
cleaner separation between choosing an end and knowing how to pursue it.

## 2. Design principles

### 2.1 Beliefs, never hidden truth

Utility functions and HTN preconditions operate only on `PlannerState`, which
is synthesized from the unit's local perception and components. Neither layer
may query enemy ground truth directly.

If a decision needs new information, that information must enter through the
belief-synthesis pipeline and be represented as a unit-local estimate.

### 2.2 Utility chooses goals; HTN decomposes them

The utility layer does not select concrete `BoundOperator`s. It selects a
high-level goal such as `Engage` or `ExecuteAssignedTask`.

The HTN does not choose among globally competing goals. It starts from the task
root associated with the selected goal and determines how to realize it.

### 2.3 Feasibility and desirability are distinct

A goal has both:

- **Feasibility:** can this goal meaningfully be attempted?
- **Utility:** how desirable is this feasible goal in the current belief state?

Examples:

- `Engage` is infeasible without a bindable hostile or ammunition.
- Low health may reduce `Engage` utility without making engagement impossible.
- `ExecuteAssignedTask` is infeasible without an active assignment.
- Immediate danger may reduce assignment utility without categorically
  invalidating the assignment.

Hard conditions should represent impossibility or genuine invariants, not
preferences disguised as booleans.

### 2.4 Commitment is part of rational agency

The unit must not switch goals whenever two scores cross by a tiny amount.
Current-goal commitment, switching costs, and minimum commitment time provide
behavioral coherence.

Emergency behavior should arise primarily from a large utility advantage and
high urgency, not by recreating a hidden fixed priority list.

### 2.5 Decisions remain legible

Every selected and rejected goal should expose its total utility and the
considerations that produced it. A bare score is insufficient for debugging.

The decision trace should be able to answer:

- which goals were feasible;
- how each feasible goal scored;
- why the winner was selected;
- why a candidate did or did not interrupt the current goal;
- how the selected goal decomposed into HTN steps.

## 3. Conceptual model

```text
PlannerState
    |
    v
Goal evaluation
    - reject infeasible goals
    - score feasible goals
    - rank by utility
    |
    v
Commitment arbitration
    - compare winner against re-scored current goal
    - apply switching margin / minimum commitment / urgency
    |
    v
HTN planning from selected goal root
    - recursively decompose compound tasks
    - bind primitive operators
    - simulate deterministic planner effects
    - backtrack if a decomposition fails
    |
    v
PlanRunner
    - dispatch operators
    - poll completion or failure
    - preserve order provenance
    |
    v
MovementOrder / CombatOrder / task delegation
```

A goal's HTN decomposition may still contain ordered methods. Those local
orders answer questions such as "which valid procedure realizes this one
goal?" They no longer establish a total ordering among all ends available to
the unit.

## 4. Proposed data model

### 4.1 Goal identity

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GoalId {
    Survive,
    Engage,
    ExecuteFallback,
    ExecuteAssignedTask,
    ExecuteCommandPlan,
    Investigate,
    Idle,
}
```

These correspond initially to the methods currently installed under
`BeInfantry`.

### 4.2 Goal definition

```rust
pub type UtilityFn = fn(&PlannerState) -> UtilityScore;

pub struct Goal {
    pub id: GoalId,
    pub task: TaskId,
    pub feasible: ConditionFn,
    pub utility: UtilityFn,
}
```

`task` is the root from which HTN decomposition begins after the goal is
selected.

The domain becomes a procedural task graph plus a repertoire of selectable
goals:

```rust
pub struct Domain {
    pub tasks: Vec<Task>,
    pub goals: Vec<Goal>,
}
```

If future archetypes need substantially different utility policy over the same
procedures, goal repertoires may later be separated from `Domain`. Keeping them
together is simpler for the first implementation.

### 4.3 Explainable utility scores

```rust
pub struct UtilityScore {
    pub total: f32,
    pub urgency: f32,
    pub considerations: Vec<ConsiderationScore>,
}

pub struct ConsiderationScore {
    pub name: &'static str,
    pub input: f32,
    pub contribution: f32,
}
```

All normalized inputs and final scores should have documented ranges, normally
`0.0..=1.0`. Score construction should reject or clamp NaN and non-finite
values so sorting remains deterministic.

`urgency` controls how readily a candidate may interrupt an existing
commitment. It does not directly replace utility and should not become a second
implicit priority list.

### 4.4 Selected plan

```rust
pub struct SelectedPlan {
    pub goal: GoalId,
    pub score: UtilityScore,
    pub plan: Plan,
}
```

`PlanRunner` records the adopted goal and adoption time:

```rust
pub struct PlanRunner {
    pub goal: GoalId,
    pub plan: Plan,
    pub current: usize,
    pub step_state: StepState,
    pub last_state_digest: PlannerStateDigest,
    pub adopted_tick: u64,
}
```

The runner need not treat the score at adoption as permanently valid. During
reconsideration, the current goal is re-scored against current beliefs.

## 5. Goal selection

### 5.1 Selection algorithm

For each deliberation:

1. Evaluate each goal's feasibility condition.
2. Score every feasible goal.
3. Sort feasible goals by descending utility.
4. Attempt HTN decomposition from the highest-scoring goal's task root.
5. If decomposition fails, record the failure and try the next goal.
6. Adopt the first successfully decomposed goal allowed by commitment policy.
7. Fall back to `Idle` if no other goal yields a valid plan.

Sketch:

```rust
pub fn select_plan(domain: &Domain, state: &PlannerState) -> Option<SelectedPlan> {
    let mut candidates = domain
        .goals
        .iter()
        .filter(|goal| (goal.feasible)(state))
        .map(|goal| (goal, (goal.utility)(state)))
        .collect::<Vec<_>>();

    candidates.sort_by(|a, b| b.1.total.total_cmp(&a.1.total));

    for (goal, score) in candidates {
        if let Some(plan) = plan_task(domain, goal.task, state) {
            return Some(SelectedPlan {
                goal: goal.id,
                score,
                plan,
            });
        }
    }

    None
}
```

Deterministic tie-breaking must be explicit. Goal registration order or
`GoalId` may break exact score ties, but the selected policy must be documented
and tested. Tie-breaking is not intended to encode broad priority; it only
stabilizes equal results.

### 5.2 Why decomposition follows scoring

A coarse feasibility condition may pass while a deeper primitive precondition
or operator binding fails. For example, `Engage` may appear feasible from a
contact summary but fail to bind a valid target.

The selector therefore ranks goals first and then asks the HTN to validate and
decompose them in that order. Planning failure falls through to the next
candidate without mutating real state.

### 5.3 Planner API change

The current planner always starts from `domain.root`. Generalize it to accept a
specific root:

```rust
pub fn plan_task(
    domain: &Domain,
    root: TaskId,
    state: &PlannerState,
) -> Option<Plan>;
```

The existing DFS, scratch-state effects, and backtracking behavior remain
unchanged.

The MTR remains useful for tracing method choices and comparing alternative
procedures within a goal. It no longer decides whether one high-level goal may
interrupt another.

## 6. Utility considerations

### 6.1 Initial scoring inputs

A first vertical slice can use existing `PlannerState` fields:

#### Survive

- inverse health fraction;
- whether the unit believes it is under fire;
- hostile confidence and freshness;
- whether retreat can bind a useful destination.

#### Engage

- visual-contact freshness;
- hostile confidence;
- health fraction;
- ammunition availability;
- whether the unit is under fire.

#### ExecuteFallback

- active fallback target;
- distance from the target;
- assignment expiry context;
- immediate believed threat.

#### ExecuteAssignedTask

- active assignment;
- distance from the station;
- time already committed;
- immediate believed threat and health.

#### ExecuteCommandPlan

- command responsibility;
- remaining delegation work;
- plan freshness;
- immediate personal threat.

#### Investigate

- contact recency;
- contact confidence;
- distance to last known position;
- current danger.

#### Idle

A small nonzero baseline makes `Idle` a stable fallback without allowing it to
beat purposeful behavior under ordinary conditions.

### 6.2 Future belief inputs

The existing state is sufficient to prove the architecture but too sparse for
rich contextual behavior. Likely additions include:

```rust
pub struct PlannerState {
    // Existing fields...

    pub ammo_frac: f32,
    pub estimated_threat: f32,
    pub nearby_ally_strength: f32,
    pub nearby_hostile_strength: f32,
    pub distance_to_cover_m: Option<f32>,
    pub suppression: f32,
    pub morale: f32,
    pub assignment_urgency: f32,
}
```

Each field must be synthesized from information available to the unit. For
example, `nearby_hostile_strength` is an estimate from remembered contacts, not
a query over all hostile entities.

### 6.3 Response curves

Continuous considerations should eventually support reusable curves:

```rust
pub enum Curve {
    Linear,
    InverseLinear,
    Quadratic,
    Logistic {
        midpoint: f32,
        steepness: f32,
    },
}
```

A retreat-health curve can rise sharply below roughly 35% health without making
35% an absolute doctrinal boundary.

The initial implementation should prefer explicit scoring functions until
repeated patterns justify a generic curve framework. Prematurely building a
utility DSL would add machinery before the scoring vocabulary is understood.

### 6.4 Composition

Weighted sums allow considerations to compensate for one another and are a
reasonable default:

```text
utility = sum(weight_i * curve(input_i))
```

Multiplicative terms may represent soft gates where one near-zero input should
strongly suppress the result:

```text
engage desirability
    = tactical value
    * target confidence
    * weapon readiness
```

True impossibility still belongs in feasibility, not in a tiny utility score.

## 7. Commitment and interruption

### 7.1 Re-score the current goal

When relevant beliefs change, compare:

```text
candidate goal utility under current beliefs
```

against:

```text
current goal utility under current beliefs
```

Do not compare the candidate only against the current goal's stale adoption
score.

If the current goal becomes infeasible or its active plan fails, its commitment
ends and normal selection resumes.

### 7.2 Hysteresis

A candidate replaces the current goal only when it wins by a meaningful margin:

```text
candidate utility > current utility + switch margin
```

```rust
pub struct DeliberationPolicy {
    pub switch_margin: f32,
    pub minimum_commitment_ticks: u64,
}
```

This prevents small score fluctuations from producing oscillation.

### 7.3 Minimum commitment

A newly adopted goal receives a short minimum commitment period. During that
period, ordinary alternatives cannot interrupt it.

This should remain short enough that units respond to tactical changes. It is a
stability mechanism, not a simulation of stubbornness. Longer-term differences
in discipline or morale should eventually enter utility policy explicitly.

### 7.4 Urgency

Candidate urgency may reduce the effective switching margin:

```text
effective margin = base margin * (1 - candidate urgency)
```

A mildly preferable investigation should not interrupt movement to a station.
An acute survival opportunity may interrupt immediately.

Hard overrides should be rare and reserved for genuine invariants. Broad rules
such as "survival always interrupts" would recreate the fixed root priority
under another name.

### 7.5 External player orders

Existing provenance arbitration remains intact:

- player movement orders block conflicting autonomous movement writes;
- player combat orders block conflicting autonomous combat writes;
- one lane does not unnecessarily suppress autonomous activity in another;
- HTN-issued orders and their provenance are inserted and removed together.

Goal selection may continue while an external order is active, but a candidate
whose plan conflicts with that order must not be dispatched. The first
implementation can preserve the current plan-level conflict checks.

## 8. Deliberation cadence and state digest

`PlannerStateDigest` currently captures categorical changes such as health
bands and fresh/stale contacts. Utility may change materially within those
bands.

Use two complementary triggers:

1. **Digest-driven deliberation** for discrete and event-like changes.
2. **Low-frequency periodic deliberation** for gradual score changes.

Utility-relevant continuous beliefs should be quantized into stable bands, for
example:

```rust
pub struct PlannerStateDigest {
    // Existing fields...

    pub health_decile: u8,
    pub contact_confidence_band: u8,
    pub threat_band: u8,
    pub ammo_band: u8,
}
```

The digest should detect decision-relevant change without embedding raw floats
or causing replanning every simulation tick.

Re-scoring goals is cheaper than HTN decomposition. A later optimization may
score on every deliberation trigger but only invoke `plan_task` when:

- the winning goal changes;
- the current plan fails;
- the current decomposition becomes invalid;
- or a materially better candidate passes commitment arbitration.

## 9. Execution behavior

Once a goal has produced a plan, the existing executor remains largely
unchanged:

1. `start_pending_steps` dispatches the current `BoundOperator`.
2. Gameplay systems execute the resulting orders.
3. `advance_plan_execution` polls for running, success, or failure.
4. Terminal steps clear their HTN-sourced orders and advance or tear down the
   runner.
5. Plan completion removes the runner and causes fresh goal selection.

Planner effects remain counterfactual simulation only. Deterministic effects,
such as assuming arrival after a planned move, may update scratch state during
decomposition. Stochastic outcomes, such as whether firing kills or suppresses
a target, remain the responsibility of gameplay resolution and later belief
synthesis.

## 10. Trace and observability

Extend `DecisionTrace` with utility-specific events. Possible shapes:

```rust
TraceEvent::GoalsEvaluated {
    candidates: Vec<GoalEvaluationTrace>,
}

TraceEvent::GoalAdopted {
    goal: GoalId,
    utility: f32,
}

TraceEvent::GoalRetained {
    current: GoalId,
    current_utility: f32,
    challenger: GoalId,
    challenger_utility: f32,
    required_margin: f32,
}

TraceEvent::GoalDecompositionFailed {
    goal: GoalId,
}
```

A candidate trace should include feasibility and consideration contributions:

```text
Selected Engage: 0.74
  contact confidence: +0.31
  immediate threat:   +0.25
  health:             +0.14
  ammo scarcity:      -0.06

Retained over Survive: 0.62
  required switch score: 0.79
```

As with existing trace events, semantically identical consecutive events should
be suppressed to avoid noise and unnecessary ECS change detection.

## 11. Proposed module layout

An eventual layout could be:

```text
src/ai/
├── htn/
│   ├── domain.rs       # procedural tasks, methods, goal task roots
│   ├── planner.rs      # decomposition from a supplied TaskId
│   ├── executor.rs     # selected-plan execution
│   ├── operators.rs
│   ├── state.rs
│   └── soldier.rs      # infantry procedures
│
└── utility/
    ├── mod.rs
    ├── goal.rs         # GoalId, Goal, UtilityScore
    ├── selector.rs     # feasibility, scoring, ranking
    ├── curves.rs       # reusable response curves
    └── infantry.rs     # infantry goal policy
```

For the first vertical slice, utility types may live under `src/ai/htn/` to
avoid over-partitioning a small implementation. Split the module after the
interface stabilizes.

## 12. Migration plan

### Phase 1: Structural separation

1. Add `GoalId`, `Goal`, `UtilityScore`, and `SelectedPlan`.
2. Generalize `plan()` into `plan_task(domain, root, state)`.
3. Replace `Domain.root` with a collection of goal definitions and task roots.
4. Remove the `BeInfantry` compound root.
5. Register its seven former methods as independently selectable goals pointing
   at their existing subtasks.
6. Preserve all current primitive tasks, bindings, effects, and operators.

At the end of this phase, simple utility functions may deliberately reproduce
approximately current behavior. The purpose is to establish the architectural
seam before tuning behavior.

### Phase 2: Utility arbitration

1. Implement feasible-goal filtering, scoring, sorting, and decomposition
   fallback.
2. Store the adopted `GoalId` and tick in `PlanRunner`.
3. Replace MTR-based cross-goal adoption with re-scoring and hysteresis.
4. Add minimum commitment and deterministic tie-breaking.
5. Preserve existing external-order conflict checks.

### Phase 3: Legibility and tuning

1. Record consideration-level score breakdowns in `DecisionTrace`.
2. Add utility-relevant quantization to `PlannerStateDigest`.
3. Add low-frequency periodic reconsideration.
4. Tune weights and curves from trace evidence and scenario tests.
5. Add debug UI showing current goal, score, competitors, and active HTN plan.

### Phase 4: Richer tactical beliefs

1. Add estimated threat and force-balance beliefs.
2. Add ammo fraction, cover access, suppression, morale, and assignment urgency
   as gameplay semantics support them.
3. Keep all estimates local to the unit's perception and communications.
4. Revisit generic response curves only after repeated scoring patterns emerge.

## 13. Testing strategy

### 13.1 Utility unit tests

- infeasible goals are not scored or selected;
- score functions remain finite and within their documented range;
- lower health smoothly increases survival utility;
- fresher, more confident contacts increase engagement utility;
- assignment urgency can outweigh moderate danger;
- extreme danger can outweigh a routine assignment;
- exact ties use deterministic tie-breaking.

### 13.2 Hybrid-selection tests

- the highest-utility decomposable goal is selected;
- HTN failure in the highest-scoring goal falls through to the next candidate;
- `Idle` is selected when no purposeful goal is feasible;
- selected goals begin decomposition from the correct task root;
- HTN method backtracking still works within a selected goal.

### 13.3 Commitment tests

- a small challenger advantage does not interrupt the current goal;
- a challenger exceeding the switch margin does interrupt;
- minimum commitment suppresses ordinary early switching;
- high urgency can reduce the switching threshold as designed;
- an infeasible or failed current goal is released immediately;
- score changes do not cause tick-to-tick oscillation.

### 13.4 Integration tests

- goal adoption creates the expected HTN-sourced gameplay order;
- player order provenance still blocks only the conflicting lane;
- plan completion causes fresh goal selection;
- belief changes trigger reconsideration through the digest or periodic path;
- traces explain selection, retention, interruption, decomposition, and action.

### 13.5 Scenario tests

Create deterministic scenarios demonstrating the flexibility sought by this
change:

1. A wounded, isolated soldier retreats from a strong contact.
2. A similarly wounded soldier with strong nearby support continues engaging.
3. A healthy soldier abandons a routine station assignment under overwhelming
   immediate threat.
4. A disciplined or high-urgency assignment wins under moderate danger.
5. A stale low-confidence contact does not repeatedly interrupt an active plan.
6. Small score fluctuations near a decision boundary do not produce
   oscillation.

## 14. Risks

### 14.1 Hidden doctrine in weights

Poorly documented weights can be less legible than method order. Mitigate with
named considerations, score breakdowns, narrow normalized ranges, and scenario
tests.

### 14.2 Oscillation

Closely matched goals may alternate as beliefs change. Mitigate with hysteresis,
minimum commitment, periodic rather than per-tick deliberation, and explicit
urgency.

### 14.3 False precision

A score such as `0.734` may imply more epistemic precision than the underlying
beliefs support. Treat utilities as comparative control signals, not measured
moral quantities. Quantize debug display where useful.

### 14.4 Combinatorial tuning

More considerations create more interactions. Begin with few inputs per goal,
add dimensions only to explain observed behavioral failures, and protect intent
with scenario tests.

### 14.5 Expensive reconsideration

Scoring every goal and decomposing every candidate every tick would waste work.
Use digest and periodic triggers, separate cheap scoring from decomposition,
and only produce a new plan when commitment arbitration permits adoption.

### 14.6 Utility collapse into hard rules

Excessive feasibility gates and emergency overrides can recreate the original
fixed priority system. Hard conditions should represent genuine impossibility
or invariant policy; ordinary tactical preference belongs in utility.

## 15. Non-goals

This design does not initially attempt to provide:

- learned utility functions or reinforcement learning;
- globally optimal planning over probabilistic world transitions;
- enemy ground-truth access;
- squad-level omniscient blackboards;
- utility selection of individual primitive operators;
- automatic tuning of weights;
- a generic visual utility-authoring DSL.

The proposal is a pragmatic hybrid, not a full expected-utility or POMDP
planner. It uses authored, explainable heuristics over local beliefs to select
an intention, then relies on the existing HTN and executor to realize it.

## 16. Decision

Adopt the following architectural boundary:

> Utility determines what presently matters most. HTN determines what pursuing
> it entails.

The first implementation should preserve the current behavior approximately,
prove the selection/decomposition seam, and add traceability before introducing
richer tactical considerations. Behavioral flexibility should then be added
incrementally through unit-local beliefs and scenario-driven tuning rather than
through a new proliferation of ordered HTN root methods.
