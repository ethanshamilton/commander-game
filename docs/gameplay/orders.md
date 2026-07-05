# Order provenance

`UnitOrder` and `CombatOrder` stay separate components (orthogonal: a unit can
be moving *and* firing, and each is read by a different system set). What is
unified is provenance — *who* issued the order — via `OrderProvenance<O>`, a
marker-typed generic component so arbitration logic works over any order kind
without duplicating it per order type.

Three sources:

- **Player** — a direct player directive. Preempts autonomous (HTN) writes to the same order lane.
- **Htn** — issued by the unit's own HTN executor. Cleared by the executor
  once the step completes/fails, or if the planner tears down the runner.
- **Doctrine** — a default posture (e.g. spawn-time `CombatOrder::HoldFire`,
  or combat resolution decaying a spent `FireAt` back to `HoldFire`). Never a
  directive, so it never suppresses planning.

**Invariant:** `OrderProvenance::<O>` is present if and only if `O` is present
on the same entity. Every site that inserts, removes, or overwrites an order
component must do the same to its provenance in the same command batch.

**Arbitration rule:** an order counts as "external" for its own order lane if
and only if its source is `Player`. Because `UnitOrder` and `CombatOrder` are
orthogonal, a player `UnitOrder::MoveTo` blocks autonomous movement/hold steps
that would overwrite the move order, but it does not block an autonomous
`CombatOrder::FireAt`. Doctrine and Htn sources never count as external.
