# NEXT Implementation Sketches

Brief, provisional approaches for every item in ROADMAP's **Next** category. These are direction-finders, not settled implementation plans.

## 1. MAP: Terrain types

Add a surface/material layer alongside the existing elevation layer in `gameplay/terrain.rs`. Elevation answers height; terrain type answers traversal and presentation properties.

Possible model:

```rust
pub enum TerrainKind { Ground, Road, ShallowWater, DeepWater, Mud }

pub struct TerrainProperties {
    pub movement_multiplier: f32,
    pub traversable: bool,
    pub visibility_multiplier: f32,
}
```

Store authored terrain as a cell raster or regions in `MapDefinition`, then compile it into a runtime `TerrainField` owned by `BattlefieldMap`. Expose queries such as `kind_at_m`, `movement_cost_at_m`, and `is_traversable`. `move_units` should sample the traversed segment rather than only its endpoint, preventing units from skipping narrow water barriers at high speed.

Start with:

- Ground: ordinary movement.
- Road: faster movement.
- Shallow water: slower movement.
- Deep water: impassable for infantry.

Render cells/regions through the map rendering module, but keep colors and sprites separate from simulation properties. Add boundary, out-of-map, and mixed-segment tests. This should be designed to deserialize cleanly when maps become data-driven.

## 2. MAP: Structures

Model structures as mission-scoped world entities rather than terrain types. A tree, wall, or building has identity, geometry, and one or more gameplay capabilities:

```rust
#[derive(Component)] struct Structure;
#[derive(Component)] struct ObstacleShape(/* circle, segment, polygon */);
#[derive(Component)] struct VisionOccluder;
#[derive(Component)] struct MovementBlocker;
#[derive(Component)] struct Cover { protection: f32 };
```

Initial slice:

1. Add trees as circular movement blockers/visual occluders.
2. Add walls as line-segment or thin-rectangle blockers.
3. Insert structures into a spatial index so movement, perception, and combat do not scan every structure.
4. Clip movement against blockers and test line-of-sight rays against occluders.
5. Render from the same authored geometry.

Avoid one giant `StructureKind` match in every system. Convert authored kinds into capability components during map instantiation. Cover can follow after collision and LOS work; it should affect shot resolution based on the ray and target pose, not be a flat buff attached to nearby soldiers.

## 3. UI: Command tree view

Add a mission-screen panel or modal that recursively presents the portion of `CommandForest` reachable from the current `PlayerControlledUnit`. Each node should show stable unit name/ID, rank, life/contact status, current assigned plan, and direct children.

Key constraint: the view must use `PlayerTacticalKnowledge` for status and position. The topology may come from the command forest, but it must not expose fresh health/death information that has not reached the player through comms.

Likely pieces:

- `CommandTreeViewState` resource for open/closed and expanded nodes.
- A flattened `VisibleCommandNode { entity, depth, ... }` projection rebuilt when topology or relevant player knowledge changes.
- Scrollable Bevy UI rows with indentation and expand/collapse controls.
- Left-click selects the represented unit; right-click opens contextual actions.
- Plan assignment submenu emits the existing `CommandPlanAssignmentRequested` rather than mutating assignments directly.
- Succession/reassignment indicators consume command-change events but display only when player knowledge permits.

Keep tree projection as a pure function and test roots, deep trees, orphans, stale/dead knowledge, and control transfer. Large-tree virtualization can wait until profiling shows it is needed.

## 4. COMMAND: Richer squad organization

The minimal succession prerequisite now belongs to NOW: a mission-scoped `Squad` contains an ordered roster, current leader, and revision, while each soldier has a `MemberOfSquad` reverse link. Roster order defines succession and formation ordering; `CommandForest` still records current individual authority.

The remaining NEXT work is richer organization:

- plan assignment targets the stable squad entity while transmission resolves its current living leader;
- explicit membership-transfer and reinforcement APIs increment the squad revision;
- tactical roles, fireteams, staff billets, and nested organizational elements;
- UI projections and player-knowledge rules for squad state;
- data-driven squad definitions once mission assets leave source code.

Do not create a magical shared “squad brain.” Individual soldiers retain beliefs and receive packets. The squad entity is organizational truth and stable identity, not collective cognition.

## 5. COMBAT: Stances

Add an explicit `Stance` component:

```rust
pub enum Stance { Standing, Crouching, Prone }
```

Derive gameplay effects through one tuning table:

- eye/sensor height;
- movement-speed multiplier;
- visual signature and chance-to-hit profile;
- transition duration;
- allowed weapon behavior;
- cover interaction.

Avoid independently mutating `EyeHeight`, `Mobility`, and signatures when stance changes, as that invites drift. Systems should query stance and calculate effective values, or a single synchronization system should own derived components.

Add `ChangeStance` as an HTN-bound operator with a timed transition component. Initial doctrine might crouch while holding under threat, go prone under fire, and stand to move quickly. Player micro-control can emit intent through the existing order/packet path later.

Rendering needs simple pose/color/shape differentiation. Perception and combat tests should verify eye-height LOS, movement speed, transition interruption, and hit resolution. Tune only after structures/cover establish what posture is relative to.

## 6. AI: Basic search pattern while holding

Extend hold behavior from static `MovementOrder::Hold` into a small observation routine. A unit at an assigned station should periodically scan a sector without abandoning the station.

Minimal implementation:

- Add a `SearchPatternProgress` component containing pattern identity, next heading index, and next scan tick.
- Generate deterministic headings around the station's assigned facing, e.g. center, left, center, right.
- Add a `FaceHeading`/`ObserveSector` HTN operator that changes heading but not position.
- Advance only after a dwell interval so the visual sensor actually samples each direction.
- Reset when the assigned task/station changes.

Priority remains roughly:

```text
Survive > Engage > Fallback > MoveToAssignedStation > SearchWhileHolding > Investigate > Idle
```

Fresh contact interrupts scanning; after engagement, the soldier resumes from a sensible heading. Search decisions use perception memory and assigned sector data, never hostile ground truth. Later variants can add short patrols, expanding searches, or coordinated sector allocation.

## 7. TOOLS: Data-driven missions and maps

Replace compiled constants in `src/missions.rs` and `src/maps.rs` with serialized assets, likely RON initially because it maps naturally to Rust enums and is pleasant to hand-author.

Suggested split:

- `assets/maps/*.map.ron`: dimensions, elevation source, terrain regions/cells, and structures.
- `assets/missions/*.mission.ron`: map asset reference, units, squads, command assignments, objectives, tutorial data.

Derive `Serialize`/`Deserialize` on authoring-only types and change borrowed `&'static` slices/strings to owned `Vec`, `String`, or asset handles. Keep runtime ECS types separate from file schemas. The load pipeline should be:

```text
Asset load -> schema parse -> semantic validation -> resolved definition -> runtime instantiation
```

Validation needs useful path-aware errors for duplicate IDs, unknown references, cycles, invalid geometry, bad objective references, and unsupported schema versions. Add a loading/error state before mission brief rather than panicking when an asset is unavailable.

Include `schema_version` from day one and ship the current demo content as fixture files. Tests should deserialize every checked-in asset and validate it in CI. Do not build the map/mission editors yet; they should eventually produce these same schemas.

## 8. TOOLS: Tuning resource

Create a typed `GameplayTuning` resource with nested sections rather than a flat bag of hundreds of values:

```rust
pub struct GameplayTuning {
    pub perception: PerceptionTuning,
    pub combat: CombatTuning,
    pub movement: MovementTuning,
    pub comms: CommsTuning,
    pub planning: PlanningTuning,
    pub ui: UiTuning,
}
```

Inventory constants first with `rg 'const '` and classify them:

- **Gameplay tuning:** ranges, cooldowns, confidence decay, arrival epsilon, formation spacing, TTLs.
- **Structural constants:** array sizes, protocol limits, simulation phase count.
- **Presentation/layout:** panel widths and colors.

Only the first category must move immediately. Structural invariants should remain constants, while UI theme/layout may deserve a separate resource.

Provide `Default` values matching current behavior, insert the resource at app startup, and migrate one subsystem at a time. Pure helper functions should accept the relevant tuning subsection explicitly, which keeps tests deterministic and avoids hidden global access. Eventually deserialize overrides from an asset and snapshot the active tuning/version into diagnostics or replay metadata.

Validate finite/positive ranges and cross-field constraints. Avoid hot reload until systems clearly handle mid-mission parameter changes.

## 9. DEV: Faster development profile

Measure before changing settings: capture clean `cargo check`, incremental `cargo check`, and link times using `cargo build --timings`.

Potential experiments:

- Ensure Bevy dynamic linking is enabled only for the dev workflow; rn it is enabled directly in `[dependencies]`, making the `dev` feature redundant.
- Compare project `opt-level = 0` against the current `1`; keep dependencies at `3` if runtime performance needs it.
- Try `debug = 1` or `debug = "line-tables-only"` to reduce artifact/link size while retaining useful backtraces.
- Keep `incremental = true` explicitly for dev.
- Use the platform's faster linker (`lld` or `mold`) through a documented, preferably opt-in `.cargo/config.toml` setup.
- Split very large, frequently edited modules such as `packets.rs` and HTN executor/synthesis if codegen invalidation measurements justify it.
- Use `cargo check` as the default inner loop and reserve full build/clippy for checkpoints.

Do not cargo-cult flags: record timings and retain only changes that improve this repo on supported machines. Keep release profile and CI behavior separate. Document required linker installation so a fresh checkout does not mysteriously fail.

## 10. DIAGNOSTICS: Evaluate `bevy-inspector-egui`

Run this as a contained dev-only spike. Add a Cargo feature such as `inspector` with a Bevy-0.19-compatible `bevy-inspector-egui` dependency, then register its quick world inspector only when that feature is enabled.

Reflect/register a focused initial set:

- `SimulationClock` and `SimulationPerf`;
- selected soldier transform/position, health, stance, orders, and planner belief;
- `CommandForest` or a read-only projected representation;
- command plans, assignments, packet inbox/outbox counts;
- `GameplayTuning` once it exists.

Questions the spike should answer:

1. Does it compile cleanly with the current Bevy version and dynamic-link setup?
2. What is its compile/link-time cost?
3. Can mutable inspection violate invariants—for example, corrupt the command forest or insert impossible plan geometry?
4. Does it perform acceptably with many entities?
5. Is it materially better than the existing purpose-built diagnostics and AI debug panels?

Default sensitive resources to read-only or expose safe proxy resources; arbitrary ECS mutation is useful but footgunny. The inspector must never ship enabled by default or become a gameplay dependency. Keep it if it accelerates debugging enough to justify build cost; otherwise remove the spike without sunk-cost sentimentality.

## Cross-item ordering

A plausible order that minimizes rework:

1. Tuning resource and dev-profile measurements.
2. Data-driven map/mission schemas.
3. Terrain types, then structures and spatial queries.
4. Richer squad organization and command tree UI, building on the minimal NOW roster.
5. Stances, once cover/occlusion semantics exist.
6. Search pattern, using stance and assigned squad sectors where available.
7. Inspector spike at any point as a dev-only experiment, ideally after tuning/squad data offer useful inspection targets.

The dependencies are soft except where noted; small vertical slices are preferable to a grand unification detour.
