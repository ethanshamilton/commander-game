# Spatial

Core spatial state and broad-phase indexing for simulation entities.

`BattlefieldPosition` is the single runtime position component for battlefield entities. It stores horizontal battlefield coordinates in meters. Terrain height is derived from the active map terrain rather than stored directly on actors.

`BattlefieldSpatialGrid` is a dense, map-bounded simulation index rebuilt after movement and before sensors on each running simulation tick. It is intentionally separate from the rendered battlefield grid: render spacing is a presentation choice, while spatial-index cell size is a performance tuning choice.

Each soldier, including a corpse, occupies exactly one spatial cell. Range queries visit every cell intersecting the queried circle's axis-aligned bounds and can therefore return false-positive candidates. Callers must still perform exact distance and domain-specific checks. The broad phase must never omit an entity that could pass those checks.

The initial cell size is 50 meters. The index rebuild is deliberately simple and linear: cells retain their allocations, are cleared, and are repopulated from current `BattlefieldPosition` components. Incremental maintenance should only replace this after measurement demonstrates a need.

Future air or elevated units should extend this spatial model with altitude rather than adding a parallel actor position component.
