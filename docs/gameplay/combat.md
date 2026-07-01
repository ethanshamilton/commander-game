# Combat

Combat v1 resolves shots probabilistically in the fixed simulation, then spawns visual-only tracers as receipts of those resolved shots.

The combat truth is immediate ECS state mutation. Tracers do not perform collision and should not determine hit/miss outcomes.

```text
combat sim: shooter + target + context -> P(hit) -> roll -> hit/miss/effects
visuals:    resolved shot -> moving tracer along chosen shot line
```

Combat orders are separate from movement orders so a unit can eventually move and fire at the same time.
