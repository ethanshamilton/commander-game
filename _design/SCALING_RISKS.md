# Scaling Risks

Ranked roughly by when each wall gets hit as unit count / scenario length grows.
Verified against code as of v0.6.1. Rough estimate: perception is the first hard
wall at ~300–500 units; memory growth degrades long sessions at *any* unit count.

## 1. Perception is the first hard wall

- **1A: Pairwise perception is `O(N²)` with LOS raycast in the inner loop**
  - `update_visual_perception` runs N observers × N targets, and each in-cone
    pair pays a terrain LOS walk of `distance/2` bilinear `height_at_m` samples
    (up to ~75 at max visual range). Auditory is another N² pass.
  - Estimate: ~3M height queries/tick at N=500 ≈ over the 50ms tick budget on
    its own, before comms, AI, or combat run.
  - Mitigation status: a dense 50m `BattlefieldSpatialGrid` now rebuilds after
    movement and supplies conservative nearby candidates to visual and auditory
    perception. Exact range/FOV/LOS checks remain unchanged. Stress testing and
    cell-size tuning are intentionally deferred.

- **1B: Corpses permanently stay in the perception target set**
  - The targets query deliberately includes `Option<&Dead>` so bodies remain
    observable. Dead units are never despawned (epistemic ambiguity).
  - Consequence: the N² term scales with *cumulative deaths*, not live units.
    Battles get monotonically slower as they progress — the opposite of the
    usual RTS profile.
  - Mitigation: corpse-specific components and separate, cheaper handling;
    eventual cleanup/archive policies for old bodies.

- **1C: Fixed-timestep death spiral is the failure mode**
  - When a fixed tick overruns, Bevy runs extra fixed steps to catch up, which
    overruns further. The sim doesn't drop fps — it falls behind wall-clock
    and then collapses.
  - `SimulationPerf.utilization` already exists to see this coming; treat
    sustained utilization > ~0.7 as the alarm threshold.
  - Mitigation: drop perception (and eventually comms graph rebuild) below
    20Hz with per-unit phase staggering (unit i senses on ticks where
    `i % k == tick % k`).

- **1D: LOS work is massively duplicated**
  - `has_line_of_sight` re-walks the same terrain for every overlapping
    sightline; nothing is cached within a tick.
  - Mitigation: cache LOS per (observer, target) pair per tick, or shadowcast
    once per observer instead of ray-marching per pair.

## 2. Unbounded per-unit memory growth (any unit count, long sessions)

Everything epistemic currently grows monotonically; nothing consumes the
staleness data we already record for cleanup.

- **2A: `PerceptionMemory.contacts` is never pruned**
  - Contacts are upserted but never expire. `upsert_contact` does a linear
    `find` per detection, so per-tick sensing cost grows with *lifetime*
    contact count, not current contacts.
  - Mitigation: confidence-weighted eviction / TTL keyed off `last_seen_tick`.

- **2B: Belief synthesis is `O(C²)` in lifetime contacts**
  - `nearest_hostile_belief` linear-searches `best_per_target` per contact.
    C=200 stale contacts → ~20k comparisons per autonomous unit per tick,
    forever.
  - Mitigation: dedupe by target in one pass (HashMap), and bounded memory
    (above) shrinks C.

- **2C: `SeenPackets` grows forever**
  - Noted in code: "V1 leaves pruning to a later delivery/TTL pass." Every
    packet ID ever heard, per unit, for the whole session. `INBOX_TTL_TICKS`
    prunes inboxes but not the seen set.
  - Mitigation: ring buffer or TTL-pruned set of recent IDs.

- **2D: `PlayerTacticalKnowledge.units` only grows**
  - Epistemically defensible (stale reports are a feature), but every consumer
    pays the linear scan (below).

## 3. Linear lookups in hot paths

- **3A: Player knowledge lookup is linear, called per known unit per frame**
  - `PlayerTacticalKnowledge::get` scans `Vec<KnownUnit>`. `draw_units` calls
    `is_recently_reported` inside its loop → `O(K²)` per *rendered* frame at
    uncapped render rate, not tick rate.
  - Mitigation: `HashMap<Entity, KnownUnit>` (or index map alongside the Vec).

- **3B: Voice comms link building is `O(N·C·N)`**
  - `update_voice_comms` collects a full snapshot Vec, then per source per
    contact does `snapshots.iter().find(...)`.
  - Mitigation: `HashMap<Entity, CommsSnapshot>` per update; spatial index for
    auditory-range neighbor queries.

- **3C: ECS does not remove algorithmic costs**
  - ECS gives component filtering, locality, and scheduling. It does not make
    `O(N²)` perception or repeated linear resource lookups cheap.

## 4. Comms/packet allocation churn

Constant-factor rather than asymptotic, but compounds with N.

- **4A: 5 chained systems drain and rebuild every inbox every tick**
  - prune → deliver → relay → apply_order_commands → apply_mission_assignments
    each do `Vec::with_capacity` + drain + push-back. ~5 allocations per unit
    per tick even when idle.
  - Mitigation: mailbox with swap-remove or index cursors instead of
    drain/rebuild.

- **4B: Packet fan-out clones per receiver**
  - Voice = physical broadcast = every packet cloned to every neighbor, then
    cloned again on relay. Payloads are small now; interception/encryption
    plans will make them bigger.
  - Mitigation: `Arc<InfoPacket>` or packet store + ID references.

- **4C: Comms graph rebuilt from scratch every tick**
  - `CommsLinks.links.clear()` for all units, then `update_comms_graph` clones
    every unit's link Vec into the adjacency map.
  - Mitigation: dirty-flag rebuilds, or build adjacency directly.

## 5. Rendering / gizmos

- **5A: Gizmos are immediate-mode; everything is re-submitted every frame**
  - Topography re-draws every cached contour segment per frame. Fine at
    320×240m; at 2km maps with real contour density it's tens of thousands of
    line primitives/frame.
  - Unit glyphs, contact boxes, comms lines, order arrows all add to the same
    budget.
  - Mitigation: bake static terrain contours to a mesh with LOD; layer
    toggles; culling; player-view projections. Glyphs → instanced sprites
    eventually.

- **5B: Sensor cone recompute is ~135k `height_at_m` calls per tick while a unit
  is selected**
  - 48 segments × ray-march where step k costs k LOS samples. Constant in N,
  so fine for a single selection — but multiplies immediately if we want
  cones for a whole squad or all visible friendlies.
  - Mitigation: share the LOS cache with perception; coarser march with
    refinement; only recompute on selection/position change beyond epsilon.

## 6. AI / HTN — fine now, watch as domains grow

- **6A: Currently well-gated**
  - `PlannerStateDigest` is quantized (no raw positions/ticks), so replans
    only fire on decision-relevant change. Keep the documented contract: any
    new `PlannerState` field read by a precondition must appear in the digest,
    band/bool-quantized.

- **6B: Planning is from-scratch with state cloning and backtracking**
  - `plan()` clones the full state at entry and per failed method. Cost is
    exponential in domain depth in principle; irrelevant at current soldier
    domain size, real once squad-level compound tasks land.
  - Mitigation: plan repair/caching instead of full re-decomposition.

- **6C: Same-tick replan spikes**
  - All autonomous units deliberate in one chained system. A salient event
    (contact report delivered) triggers a same-tick replan burst across
    everyone who heard it.
  - Mitigation: phase-stagger Thinking like Sensors; cap replans per tick.

- **6D: Autonomous target selection reuses perception memory**
  - Good: no separate pairwise search. Bad: bounded by the `O(C²)` synthesis
    above. Fixing memory bounds fixes this too.

## 7. Systems-count accumulation

- **7A: Many systems scanning all soldiers adds up**
  - Each new all-units system is another `O(N)` pass at 20Hz; ~30 systems × N
    is fine until it isn't.
  - Mitigation: narrower queries, lower-frequency systems, change detection,
    merging passes that touch the same components.

- **7B: Fixed tick frequency multiplies everything**
  - Any expensive fixed-update system pays 20×/s regardless of need.
  - Mitigation: perception/comms/reporting at lower, staggered frequencies.

## 8. Architectural ceilings (feature-blocking, not perf)

- **8A: `PlayerTacticalKnowledge` is a singleton resource for one player**
  - Enemy comms interception and AI commanders (both on the roadmap) need
    per-faction or per-command-node knowledge. Hardest item here to retrofit;
    the packet/claim/staleness plumbing is already faction-agnostic, so it's
    mostly a container change, but it touches everything.
- **8B: Content is `&'static` Rust**
  - Scenarios, maps, and heightmaps are compile-time. Fine at 2 scenarios;
    the bottleneck at 20. Map/scenario editor (ROOT_NOTES) becomes
    force-multiplying early.
- **8C: `screens/scenario.rs` is becoming a UI god-object (~1.6k lines)**
  - Per-frame `collect` for the mission list, dozens of marker components.
    Maintenance scaling risk more than runtime.

## Sequencing recommendation

1. ~~Spatial grid over `BattlefieldPosition`~~ — implemented for perception;
   reuse for direct comms/target queries when those systems need it.
2. Memory TTLs/eviction for `PerceptionMemory` + `SeenPackets` (cheap; fixes
   in-session decay).
3. `HashMap` for `PlayerTacticalKnowledge` (one afternoon; fixes `O(K²)`
   rendering).
4. Tick-rate reduction + phase staggering for Sensors/Comms.
5. Defer gizmo/mesh work, HTN plan caching, and content pipeline until
   measurements say so (`SimulationPerf` phase EMA already tells us where the
   time goes).
