# NOW Plan: Shared Macro Command Plans

## Roadmap item

> **PLANS:** Plan system for use by both AI and human commanders with multiple plan types, allowing macro level command.

## Goal

Turn the existing Hold Line vertical slice into one domain API through which either a player-controlled commander or an autonomous commander can:

1. create a validated tactical plan;
2. assign that plan to one or more subordinate commanders;
3. partition the plan across those assignees;
4. transmit intent through the existing packet/comms layer; and
5. let each recipient HTN decompose its portion into subordinate tasks and concrete actions.

A plan remains intent, not an order lane. Only HTN operators may produce `MovementOrder` and `CombatOrder` for plan execution.

## Current substrate

Most of the first vertical slice already exists:

- `src/gameplay/command_plans.rs` defines plan IDs, kinds, areas, snapshots, assignments, Hold Station tasks, validation, and the assignment request.
- `src/player/plan_placement.rs` creates Hold Line plans from three map clicks.
- `src/screens/mission/plan_panel.rs` lists, renames, expires, and selects plans.
- `src/gameplay/packets.rs` transports and validates plan/task assignments.
- `src/ai/htn/leader.rs` decomposes Hold Line for one coordinator and its direct subordinates.
- `src/ai/htn/soldier.rs` executes assigned Hold Station tasks.
- Expiry fallback already regroups a squad into a wedge.

Important limitations:

- Plan creation is coupled to player UI; AI has no creation API.
- Only Hold Line has placement and decomposition behavior.
- `CommandPlanAssignees(Vec<Entity>)` records names, not per-assignee portions or assignment revisions.
- Assigning two squad leaders to one Hold Line makes both cover the entire line. It does not produce coordinated macro behavior.
- The UI assumes one selected plan and a single `PlayerControlledUnit` issuer.
- Plan/task progress is specialized to Hold Line and cannot represent waypoint-based plans.

## Design invariants

1. **One plan model for human and AI authors.** Input source changes how a draft is produced, not how it is validated, stored, assigned, or executed.
2. **Plan entities are author-local intent objects.** Recipients receive packet-safe snapshots; they do not dereference the author's plan entity.
3. **Assignment is a revisioned operational act.** Changing the assignee set can alter every assignee's portion, so affected recipients receive a newer assignment.
4. **Multi-squad assignment partitions work.** It must not silently duplicate the whole objective unless a plan explicitly requests independent execution.
5. **HTN owns decomposition and action.** Creation/assignment systems never write concrete action orders.
6. **Belief-local completion.** Scout/Clear completion uses the executing unit's observations and reports, not hidden ground truth.
7. **Determinism.** Stable unit IDs, plan IDs, and geometric ordering determine partitions and routes; Bevy entity allocation order is only a last-resort runtime tiebreaker.
8. **Coordinates remain meters.** Render-unit conversion stays at UI/rendering boundaries.

## Target model

### Source-neutral creation API

Move construction behind a gameplay request rather than spawning directly from `plan_placement.rs`.

```rust
pub struct CommandPlanDraft {
    pub label: Option<String>,
    pub kind: CommandPlanKind,
    pub area: CommandPlanArea,
    pub rally_point_m: Vec2,
    pub expires_at: Option<u64>,
}

#[derive(Message)]
pub struct CommandPlanCreationRequested {
    pub request_id: PlanRequestId,
    pub author: Entity,
    pub draft: CommandPlanDraft,
}

#[derive(Message)]
pub enum CommandPlanCreationResult {
    Created { request_id: PlanRequestId, plan: Entity },
    Rejected { request_id: PlanRequestId, reason: PlanCreationError },
}

#[derive(Component)]
pub struct CommandPlanAuthor(pub Entity);
```

The consumer validates that the author is a living soldier with command responsibility, allocates ID/label, validates geometry and expiry, and spawns the `MissionScoped` plan. Player placement and AI both emit this request.

Do not put UI concepts such as selection or preview in this API. The player adapter selects a plan after receiving `Created`; AI records the result in its commander state.

### Revisioned assignment records

Replace the bare assignee vector with records describing the operational portion sent to each commander.

```rust
#[derive(Component, Default)]
pub struct CommandPlanAssignments {
    pub revision: u64,
    pub records: Vec<CommandPlanAssignmentRecord>,
}

pub struct CommandPlanAssignmentRecord {
    pub assignee: Entity,
    pub execution_area: CommandPlanArea,
    pub issued_tick: u64,
    pub revision: u64,
}

pub struct CommandPlanAssignmentMessage {
    pub plan: CommandPlanSnapshot,
    pub execution_area: CommandPlanArea,
    pub revision: u64,
    pub issued_tick: u64,
}
```

Supersession compares `(issued_tick, revision)` lexicographically. This removes same-tick ambiguity when adding several squads or restructuring after casualties. Keep `CommandPlanId` stable across revisions.

`AssignedCommandPlan` stores both the strategic snapshot and the recipient's execution area. Planner synthesis only decomposes the execution area.

### Partition policy

Each plan kind declares whether assignment is partitioned or independent. Initial built-ins are partitioned:

- **Hold Line:** split line length into contiguous segments weighted by each squad's living participant count. Shared endpoints are allowed; no segment overlap otherwise.
- **Secure Perimeter:** convert circle/rect boundary to clockwise path distance, then allocate contiguous arcs weighted by participant count.
- **Scout Area:** split rectangle into parallel strips along its longest axis. Point/circle variants can wait until rectangle behavior is solid.
- **Clear Area:** same geometric partition as Scout Area, but different completion and engagement doctrine.

Partitioning must be a pure function:

```rust
fn partition_plan(
    kind: CommandPlanKind,
    area: CommandPlanArea,
    assignees: &[AssigneeCapacity],
) -> Result<Vec<PlanPartition>, PartitionError>;
```

`AssigneeCapacity` uses living direct-command-member count and stable `UnitId`. Sort before partitioning. Tests should prove total coverage, no unintended overlap, finite geometry, and input-order independence.

Adding/removing an assignee increments the revision, recomputes all partitions, and retransmits every changed record. Explicit cancellation packets are required for removed assignees; local mutation alone is not operational cancellation.

### Generic execution progress

Replace Hold-Line-only progress with plan/task execution state keyed by identity and revision.

```rust
#[derive(Component, Default)]
pub struct PlanExecutionProgress {
    pub identity: Option<(CommandPlanId, u64)>,
    pub delegated: Vec<DelegatedTaskKey>,
    pub phase: PlanExecutionPhase,
}

#[derive(Component, Default)]
pub struct TaskExecutionProgress {
    pub identity: Option<TaskIdentity>,
    pub waypoint_index: usize,
    pub visited: Vec<bool>,
    pub completed: bool,
}
```

Reset progress whenever assignment identity/revision changes. Keep side effects idempotent across replans.

## Plan semantics

### 1. Hold Line

Preserve current behavior, but run squad-level decomposition against the squad's assigned line segment.

Leader decomposition:

1. collect living direct subordinates plus coordinator;
2. generate evenly spaced stations over the execution segment;
3. generate unique fallback wedge stations;
4. issue one `HoldStation` task per subordinate;
5. occupy the coordinator station; and
6. regenerate assignments when command membership changes.

This becomes the regression case for the generic APIs.

### 2. Secure Perimeter

Initial supported shape: circle. Rectangle follows using the same perimeter-path abstraction.

Leader decomposition:

1. sample enough perimeter stations for living participants;
2. space stations uniformly by path distance within the assigned arc;
3. face each station outward from the protected area;
4. send `TaskDirective::SecureStation { station, sector, fallback, expires_at }`;
5. units move, hold, and engage under existing higher-priority contact behavior.

Completion is continuous until expiry/cancellation; this is a posture, not a one-shot task.

### 3. Scout Area

Initial supported shape: normalized rectangle.

Leader decomposition:

1. partition the assigned rectangle into one lane per participant;
2. generate a deterministic boustrophedon route for each lane;
3. issue `TaskDirective::ScoutRoute { waypoints, fallback, expires_at }`;
4. subordinate HTN visits waypoints and sends ordinary contact reports as perception changes;
5. after the route, hold at the final waypoint or rally according to doctrine.

A route is complete when all of its waypoints are visited. This deliberately approximates area coverage without reading ground truth.

### 4. Clear Area

Reuse rectangle lanes/routes from Scout Area, but execution semantics differ:

1. sweep the route;
2. existing `Engage` behavior continues to outrank route traversal when fresh hostile contact exists;
3. resume the route after contact is no longer actionable;
4. mark the local task complete after route coverage and a short no-fresh-hostile observation window;
5. report completion upward with a new `TaskStatusReport` payload.

The plan coordinator considers the plan locally complete after all delegated tasks report completion and its own route is complete. Reports can be delayed or absent due to comms; the coordinator's belief may therefore differ from reality by design.

## AI commander integration

Do not create a separate “AI plan” type. Add commander deliberation above the current infantry domain:

- `CommanderGoal`/planner belief chooses a plan kind, area, rally point, expiry, and candidate subordinate commanders.
- AI operator emits `PendingCommandPlanCreation`.
- A gameplay bridge converts that intent to `CommandPlanCreationRequested`.
- Once `CommandPlanCreationResult::Created` arrives, another operator emits assignment requests through the same API the player uses.

Start with a scripted/rule-based AI commander for testability:

- if defending a location, create Secure Perimeter;
- if given a line objective, create Hold Line;
- if no recent intel in an area, create Scout Area;
- if hostile contacts are believed in an area, create Clear Area.

The commander must reason from `PerceptionMemory`/received reports and mission-provided objectives, never query hostile ground truth to choose an area.

## Player integration

Refactor placement into adapters over `CommandPlanDraft`:

- Hold Line: start, end, rally.
- Secure Perimeter: center, radius point, rally.
- Scout/Clear rectangle: first corner, opposite corner, rally.

The plan panel should:

- expose enabled plan types only when their full execution path exists;
- show author, plan revision, assignees, and each assigned portion;
- distinguish draft/selected/assigned/expired/completed states;
- report typed creation/assignment rejection reasons instead of only log warnings;
- retain keyboard cancellation and input suppression during placement.

Plan overlays render the strategic whole and, when selected, color each squad partition separately. Overlay visibility must continue to respect player knowledge and selection rather than exposing enemy-authored plans.

## Implementation sequence

### P1 — Stabilize identities and request results

- Add `PlanRequestId` and a mission-local allocator.
- Introduce creation request/result messages and `CommandPlanAuthor`.
- Route existing Hold Line placement through the request.
- Add typed assignment result messages.
- Preserve all current behavior.

**Done when:** both a test system and the UI can create the same validated Hold Line through the API.

### P2 — Revisioned multi-squad assignment

- Replace `CommandPlanAssignees` with assignment records.
- Add assignment revision to snapshots, installed assignments, planner digest, and progress identity.
- Implement pure Hold Line partitioning by living squad capacity.
- Reissue changed partitions and add explicit cancellation payloads.
- Update overlays and plan panel.

**Done when:** assigning two squads produces two non-overlapping line segments whose union is the authored line.

### P3 — Generic progress and task status

- Generalize delegation/task progress.
- Add task IDs and status report packets (`Accepted`, `InProgress`, `Completed`, `Failed`).
- Ensure report consumers use packet origin/authority and never infer status from remote ECS components.
- Add trace events for creation, assignment, decomposition, completion, and rejection.

### P4 — Secure Perimeter

- Circle placement and validation.
- Pure perimeter partition/station generation.
- Secure Station task model, packet validation, planner projection, HTN behavior, and overlays.
- Enable UI button only after end-to-end tests pass.

### P5 — Scout Area

- Rectangle placement.
- Pure rectangle partition and route generation.
- Route progress, HTN traversal, completion reports, and overlays.

### P6 — Clear Area

- Reuse rectangle/route substrate.
- Add local-belief completion window and resume-after-engagement behavior.
- Add coordinator completion aggregation.

### P7 — AI author vertical slice

- Add a deterministic AI commander component/domain.
- Have it create and assign at least one Hold Line or Secure Perimeter plan through public requests.
- Run the same behavior for Blue and Red in a headless integration test.

### P8 — Hardening

- Repartition after death/succession using command-structure change messages.
- Reject stale revisions, cancelled plans, invalid authors, cross-side recipients, and dead entities.
- Bound retained plan/task/status history.
- Update `docs/gameplay/command_plans.md`, HTN docs, and packet docs.

## Likely file changes

- Major: `src/gameplay/command_plans.rs` (split into `model.rs`, `creation.rs`, `assignment.rs`, `partition.rs`, and `tasks.rs` if it continues growing).
- Major: `src/ai/htn/leader.rs`, `soldier.rs`, `state.rs`, `synthesis.rs`, `operators.rs`, `executor.rs`.
- Major: `src/gameplay/packets.rs` (prefer payload-specific consumer submodules before adding more variants).
- Player/UI: `src/player/plan_placement.rs`, `src/screens/mission/plan_panel.rs`.
- Rendering: `src/gameplay/rendering/overlays.rs`.
- Tests/docs beside each module.

## Test matrix

- Creation parity: player adapter and AI adapter create equivalent plans.
- Validation: every kind/shape combination, NaN/infinity, degenerate geometry, expiry overflow, dead/unauthorized author.
- Partition properties: deterministic ordering, full coverage, no gaps/overlap, one assignee, uneven capacities, casualties.
- Transport: direct, relayed, unreachable, stale revision, cancellation, reassignment, same-tick revisions.
- HTN: each task creates only HTN-sourced concrete orders; survival/engagement/fallback priorities remain intact.
- Completion: route interruption/resumption, delayed status reports, expiry exactly on the boundary.
- Isolation: an AI commander cannot inspect enemy plans or ground truth through global queries.
- Mission teardown resets every allocator, plan entity, request, selection, and progress component.

## Acceptance criteria

- Human and AI commanders use the same creation and assignment APIs.
- Hold Line, Secure Perimeter, Scout Area, and Clear Area each work end to end for at least their initial shape.
- One strategic plan can coordinate multiple squads without duplicating the whole execution area.
- Revisions/cancellations are explicit and stale packets cannot restore old intent.
- Every resulting movement/combat action is HTN-sourced and traceable to an assigned plan/task.
- `cargo test`, `cargo clippy --all-targets`, and `cargo fmt --check` pass.

## Explicitly deferred

- Free-form player scripting or a general visual behavior editor.
- Learned planning/ML plan selection.
- Full OPORD/SMEAC modeling, logistics, morale, and constraints beyond area/rally/expiry.
- A squad entity abstraction; this work stays compatible with the current individual `CommandForest` until the separate squad-organization roadmap item.
