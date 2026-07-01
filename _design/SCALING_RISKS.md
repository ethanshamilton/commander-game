# Scaling Risks

- **Pairwise perception is `O(N²)`**
  - Visual and auditory perception currently compare observers against targets directly.
  - This will likely be the first major simulation bottleneck as unit count grows.
  - Future mitigation: spatial hash/grid/quadtree to query nearby targets only.

- **Player knowledge uses linear lookup**
  - `PlayerTacticalKnowledge::get` scans `Vec<KnownUnit>`.
  - Rendering and selection can become `O(N * K)` where `K` is known units.
  - Future mitigation: store known units in a `HashMap<Entity, KnownUnit>`.

- **Comms snapshot lookup is linear**
  - Voice comms currently searches snapshots with `iter().find(...)` for contact targets.
  - This can become expensive as contacts increase.
  - Future mitigation: build a `HashMap<Entity, CommsSnapshot>` per comms update.

- **Many systems scanning all soldiers can add up**
  - ECS makes systems modular, but each new all-units system adds another `O(N)` pass.
  - This is usually fine early, but can become costly with many simulation layers.
  - Future mitigation: narrower component queries, lower-frequency systems, and dirty/change filters where appropriate.

- **Autonomous target selection can become expensive**
  - Explicit combat orders are cheap, but AI-driven target choice can become another pairwise or search-heavy system.
  - Future mitigation: reuse spatial indexes and perception memory rather than searching all enemies.

- **Graph/resource operations are not automatically optimized by ECS**
  - Comms graph, command forest, and knowledge propagation are ordinary data structures.
  - Their performance depends on their internal representation.
  - Future mitigation: choose maps/sets carefully and avoid repeated linear scans.

- **Gizmo/rendering volume can become a bottleneck**
  - Drawing many overlays, contacts, comms lines, command arrows, grid lines, and topography lines may get expensive.
  - Future mitigation: layer toggles, culling, level-of-detail, and player-view projections.

- **Fixed tick frequency multiplies cost**
  - Simulation currently runs at a fixed 20 Hz.
  - Any expensive fixed-update system pays that cost every tick.
  - Future mitigation: run perception, AI planning, or reporting at lower frequencies where acceptable.

- **Dead/stale entities can accumulate**
  - Dead units are intentionally not despawned to preserve epistemic ambiguity.
  - If many dead entities remain queryable, systems must filter active units carefully.
  - Future mitigation: capability components, `Alive` filters, corpse-specific components, and eventual cleanup/archive policies.

- **ECS helps but does not remove algorithmic costs**
  - ECS provides component filtering, data locality, sparse system participation, and scheduling benefits.
  - It does not make `O(N²)` perception or repeated linear resource lookups cheap.
