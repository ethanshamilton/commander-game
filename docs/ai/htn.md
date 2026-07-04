# HTN Planner

The HTN module contains the per-unit hierarchical task network planner.

This layer is intentionally split into a pure planning core and ECS adapters:

- `state`: compact `PlannerState` belief snapshot used by the planner.
- `synthesis`: converts unit-local simulation data (`PerceptionMemory`, `Health`, `Inventory`, current orders, recent shot impacts) into `PlannerState`.
- `domain`: task, method, operator-binding, and domain definitions.
- `planner`: recursive DFS decomposition, simulated effects, plans, bound steps, and method traversal records (MTRs).

`PlannerState` is not ground-truth world state. It is a decision-oriented snapshot synthesized from what the unit can know or currently carries. The planner should not query enemy ground truth directly.

Primitive tasks bind abstract actions into `BoundOperator`s at plan time. For example, `FireAtNearestHostile` can bind to `BoundOperator::FireAt { target }` only when the planner state contains a hostile belief. This keeps doctrine-level task names separate from concrete execution orders.

The pure planner does not execute actions. Executor systems translate `BoundOperator`s into existing gameplay components such as `UnitOrder` and `CombatOrder`, track exactly which orders HTN issued, and preserve externally replaced player orders. Replanning is MTR-gated, with equal-MTR plans adopted only when their bound operators differ.
