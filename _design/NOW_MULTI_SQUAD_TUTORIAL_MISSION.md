# NOW Plan: Multi-Squad Command Tutorial Mission

## Roadmap item

> **MISSIONS:** Multi-squad command tutorial mission. This will be used to implement and test command succession.

## Goal

Add a deterministic authored mission that teaches macro command across two friendly squads and provides a repeatable vertical-slice test for leader death, succession, inherited plan intent, and dynamic redelegation.

This should be both playable and useful as an integration fixture. It must not depend on a lucky combat outcome to exercise succession.

## Dependencies

Implement after the minimum usable slices of:

1. shared/multi-assignee command plans (`_design/NOW_PLANS.md` P1–P2); and
2. command succession (`_design/NOW_COMMAND_SUCCESSION.md` C1–C6).

The mission can be authored earlier, but its succession tutorial steps should remain disabled until those mechanics exist.

## Current mission-authoring constraints

`src/missions.rs` currently supports:

- static mission ID/name/briefing/map;
- unit definitions;
- superior assignments;
- victory and defeat condition arrays.

The only mission is a symmetric single-squad firefight. Runtime instantiation also marks **every Blue Sergeant** as `PlayerControlledUnit`. That fails for a two-squad mission because player input calls `controlled.single()`.

There is no authored:

- explicit player command node;
- succession priority;
- tutorial step/script data;
- named objective area;
- deterministic trigger/action mechanism;
- non-combat tutorial completion condition.

These must be added narrowly, without prematurely building the full data-driven mission-tooling roadmap item.

## Mission concept

### Name

**Tutorial: Company Command and Succession** (working label; structurally this is a small platoon, but use the least misleading final name once rank/squad terminology is settled).

### Learning objectives

The player should learn to:

1. command through subordinate leaders instead of micromanaging riflemen;
2. create a macro Hold Line plan;
3. assign one strategic plan across two squads;
4. observe each squad receive a distinct execution segment;
5. continue operating when one squad leader is killed;
6. identify the designated successor and revised command links;
7. verify that the successor inherits the plan and redelegates stations; and
8. defeat a limited enemy attack while maintaining the line.

### Friendly order of battle

Use an explicit three-level tree:

```text
Blue Lieutenant (player-controlled command node)
├── Alpha Sergeant
│   ├── Alpha 1 (succession priority 1)
│   ├── Alpha 2 (priority 2)
│   └── Alpha 3 (priority 3)
└── Bravo Sergeant
    ├── Bravo 1 (priority 1)
    ├── Bravo 2 (priority 2)
    └── Bravo 3 (priority 3)
```

The lieutenant is physically placed behind the line but within initial voice-relay range of both squad leaders. Squad members begin close enough for orders to propagate, while spacing still makes the comms graph visible and meaningful.

If a Corporal rank is added for succession, use corporals as priority-1 successors. Otherwise use the existing Private rank plus authored `SuccessionPriority`; do not block the mission on a rank-list expansion.

### Enemy order of battle

Use one or two small Red elements with a simple deterministic attack posture. Enemy strength should demonstrate coordination without making the tutorial hostage to current very fast TTK.

Recommended first balance:

- one Red Sergeant and four riflemen;
- initial positions outside Blue visual range;
- delayed advance toward the center of Blue's line;
- enough separation that Blue has time to complete plan placement and assignment before contact.

Do not use enemy AI omniscience to drive the attack. A mission-authored initial Red plan may be created at setup through the same AI/gameplay plan API, or a narrow scripted movement objective can be used until AI commander plan creation lands.

### Map layout

Reuse `DEMO_MAP` initially unless a small dedicated compiled map is materially clearer. Add named mission regions in meters:

- `blue_staging_area` behind the intended defense;
- `defensive_line_corridor` spanning both squads;
- `alpha_sector` and `bravo_sector` for tutorial validation only;
- `enemy_approach` opposite the line;
- `blue_rally_area` behind the lieutenant.

The UI can highlight these regions during the relevant tutorial step. The authored regions are hints/validation bounds, not hidden movement orders.

## Explicit player command node

Extend `MissionDefinition`:

```rust
pub struct MissionDefinition {
    // existing fields...
    pub player_command_unit: UnitId,
    pub tutorial: Option<&'static TutorialDefinition>,
}
```

At instantiation, resolve `player_command_unit` and insert `PlayerControlledUnit` on exactly that living entity. Delete the current `Side::Blue && Rank::Sergeant` heuristic.

Validation should reject:

- unknown player command unit;
- player command unit on the wrong side;
- multiple definitions resolving to the same `UnitId`;
- player command unit absent from the command forest.

After command succession, the runtime marker can transfer even though the authored ID remains the mission's starting commander.

## Tutorial framework

Build a small event-driven tutorial sequence, not a general scripting language.

```rust
pub struct TutorialDefinition {
    pub steps: &'static [TutorialStepDefinition],
}

pub struct TutorialStepDefinition {
    pub id: TutorialStepId,
    pub title: &'static str,
    pub instruction: &'static str,
    pub completion: TutorialCondition,
    pub on_enter: &'static [TutorialAction],
    pub on_complete: &'static [TutorialAction],
}
```

Runtime resource:

```rust
#[derive(Resource)]
pub struct TutorialProgress {
    pub step_index: usize,
    pub entered_tick: u64,
    pub completed: bool,
}
```

Conditions/actions are compiled enums for NOW. This remains intentionally separate from the later data-driven mission/editor work.

### Conditions needed

- `PlanCreated { kind: HoldLine, area_within: RegionId }`
- `PlanAssignedTo { unit: UnitId }`
- `PlanAssignedToAll { units: &[UnitId] }`
- `UnitsAtAssignedStations { command_root: UnitId, fraction: f32 }`
- `UnitDead { unit: UnitId }`
- `SuccessorIs { deceased: UnitId, successor: UnitId }`
- `PlanAssumedBy { unit: UnitId }`
- `RedelegationRevisionAtLeast { unit: UnitId, revision: u64 }`
- `AllHostilesEliminated`

Conditions should consume gameplay messages where possible and inspect only stable, public runtime state otherwise. Avoid matching by display labels.

### Actions needed

- `ShowRegion(RegionId)` / `HideRegion(RegionId)`
- `PauseSimulation` / `ResumeSimulation`
- `ShowHint(&str)`
- `ArmLeaderCasualty { unit: UnitId }`
- `ReleaseEnemyAdvance`

`ArmLeaderCasualty` should not silently kill the leader immediately. It enables the controlled succession exercise described below.

## Tutorial sequence

### Step 1 — Understand the hierarchy

Instruction: select the lieutenant, Alpha leader, and Bravo leader; inspect command arrows.

Completion can be a simple `SelectedUnitsInOrder` tutorial-only condition or an explicit Continue button. Do not overfit core selection systems to this pedagogical step.

Teach that commands originate from the lieutenant and travel through comms.

### Step 2 — Create the defensive plan

Highlight the defensive corridor and rally area. Ask the player to create one Hold Line crossing the corridor with the rally point in the rear area.

Completion validates plan kind and approximate geometry, not pixel-perfect points:

- line intersects both side boundaries of the corridor;
- rally point lies in `blue_rally_area`;
- expiry is absent or long enough to finish the exercise.

If invalid, keep the plan but explain why it does not satisfy the tutorial; do not secretly rewrite it.

### Step 3 — Assign both squads

Ask the player to assign the same selected plan to Alpha and Bravo leaders.

Completion requires both assignment records at the current revision. The overlay should show two differently colored contiguous line segments.

This is the integration check that multi-squad assignment partitions work rather than duplicating the entire line.

### Step 4 — Establish the line

Resume simulation and wait until a configurable percentage (prefer 100% for deterministic no-contact setup) of Blue squad members reach current assigned stations.

Show packet/command arrows if available. Do not start the enemy advance yet.

### Step 5 — Succession event

Once the line is established, prompt:

> Alpha's leader is about to become unavailable. Observe who assumes command and how the squad reorganizes.

Use a deterministic casualty harness:

- Preferred automated test mode: issue `ScriptedCasualty { target: alpha_sergeant }` through the same lifecycle transition as combat, with cause `Tutorial`.
- Preferred player-facing mode: expose a Continue/“Trigger exercise” action, then apply the scripted casualty.
- Optional manual dev mode: select Alpha Sergeant and use the existing debug-kill input.

Do not rely on Red marksmanship to kill exactly Alpha Sergeant.

### Step 6 — Verify assumption and redelegation

Pause briefly after Cleanup so the player can inspect:

- Alpha 1 is now subordinate to the lieutenant;
- Alpha 2 and Alpha 3 are subordinate to Alpha 1;
- Alpha 1 has `AssumedCommand` and inherited the current plan portion;
- dead Alpha Sergeant is absent from the command forest;
- Alpha's delegation revision increased;
- living Alpha members received revised stations through packets.

Completion waits for `CommandSucceeded`, inherited plan installation, and acceptance/transmission of revised tasks. If comms geometry prevents delivery, provide a hint rather than bypassing the network.

### Step 7 — Defend

Release the Red advance and resume simulation. Existing combat behavior handles engagement. The successor continues coordinating under the inherited plan.

Victory: all hostile units eliminated after succession has completed.

Defeat options:

- all Blue units eliminated; or
- player command node has no successor and is dead.

Do not fail merely because the line is locally displaced during combat; that would punish the existing higher-priority survival/engagement doctrine.

### Step 8 — Debrief

Show a compact summary:

- original and successor command IDs;
- succession tick;
- inherited plan ID/revision;
- number of revised task packets sent/received;
- whether any units were unreachable;
- mission outcome.

Use existing decision traces/diagnostics as data sources rather than constructing a second hidden history.

## Mission runtime validation

Before spawning, add `MissionDefinition::validate()` covering:

- unique `UnitId`s;
- finite positions/headings;
- command assignment references exist;
- no command cycles or cross-side links;
- exactly one explicit player command node;
- sibling succession priorities are deterministic;
- tutorial unit/region references exist;
- required regions use finite, normalized geometry.

Fail fast with a useful error in dev builds. Unit-test mission definitions without starting the Bevy renderer.

## Deterministic integration harness

The tutorial should support headless advancement through the same messages used in live play.

Create a test helper that:

1. instantiates the mission into a `World`;
2. creates a valid Hold Line via `CommandPlanCreationRequested`;
3. assigns Alpha and Bravo via assignment requests;
4. advances fixed simulation passes until delegation is accepted;
5. emits the scripted Alpha leader death;
6. runs Cleanup and the following Thinking/Comms passes;
7. asserts topology, assumption, revisions, and task recipients;
8. optionally resolves/removes enemies and checks victory/debrief state.

The harness may bypass mouse/UI coordinates, but it must not bypass gameplay creation, assignment, packet, lifecycle, or succession APIs.

## Implementation sequence

### M1 — Mission schema hardening

- Add explicit `player_command_unit`.
- Add succession priority to authored units/runtime spawn.
- Add mission definition validation.
- Update the existing single-squad tutorial.
- Remove rank-based player marker assignment.

**Done when:** both current and new mission definitions instantiate with exactly one player command node.

### M2 — Author order of battle and regions

- Add the two-squad Blue hierarchy and small Red force.
- Add named tutorial regions.
- Add mission to `TUTORIAL_MISSIONS` after the single-squad mission.
- Write briefing text that states the command and succession objectives.

### M3 — Minimal tutorial runtime

- Add `TutorialDefinition`, progress resource, condition evaluator, and HUD card.
- Implement plan creation/assignment/station conditions.
- Highlight active regions through a tutorial overlay.
- Reset all tutorial state on mission exit.

### M4 — Deterministic succession exercise

- Add tutorial casualty action routed through lifecycle.
- Add succession/assumption/redelegation conditions.
- Add pause/inspect/resume flow.
- Ensure no hidden enemy succession information is displayed.

### M5 — Enemy release and mission outcome

- Hold Red force until succession setup is complete.
- Release via authored AI plan or narrow mission action.
- Require succession completion plus existing hostile-elimination condition for tutorial victory. Add a composable tutorial condition rather than changing global `AllHostilesEliminated` semantics.

### M6 — Debrief and integration test

- Add summary panel using traces/messages.
- Build headless vertical-slice test.
- Tune positions, voice ranges, timing, enemy count, and health/damage so the teaching sequence is stable.

## Likely file changes

- `src/missions.rs` (consider `src/missions/tutorial_multi_squad.rs` once static data grows).
- `src/gameplay/mission_runtime.rs`.
- New `src/gameplay/tutorial.rs` and `docs/gameplay/tutorial.md`.
- `src/actors/spawning.rs`, `src/actors/units.rs` for succession priority.
- `src/gameplay/objectives.rs` for a composed tutorial victory gate, if needed.
- Mission HUD/layout and rendering overlays.
- `docs/missions.md`, mission select/brief docs.

## Test matrix

- Definition validation catches unknown IDs, duplicate IDs, cycles, wrong-side player node, malformed regions, and unstable priority.
- Mission runtime creates exactly one `PlayerControlledUnit` despite multiple sergeants.
- Alpha/Bravo are both commandable by the lieutenant but not laterally by each other.
- One plan assignment yields distinct partitions covering the intended line.
- Scripted casualty emits the same lifecycle/succession messages as combat death.
- Alpha 1 succeeds Alpha Sergeant deterministically.
- Alpha 2/3 are reparented; Bravo topology is unchanged.
- Plan ID/expiry survive assumption; progress/revision is refreshed.
- Revised directives traverse comms and stale predecessor directives lose supersession.
- Mission cannot complete before required tutorial succession step.
- Tutorial resources, overlays, messages, and entities are clean on replay/mission exit.

## Acceptance criteria

- The mission appears in mission select and has a clear briefing.
- It contains one player commander above at least two independently led squads.
- The player can assign one macro plan across both squads and see partitioned execution.
- Alpha leader death is deterministic and uses the real lifecycle path.
- The designated successor assumes command, inherits intent, and dynamically redelegates through comms.
- The tutorial detects and explains stalled prerequisites instead of silently advancing.
- A headless integration test proves the complete creation → assignment → death → succession → redelegation path.
- Existing single-squad mission behavior remains intact.
- `cargo test`, `cargo clippy --all-targets`, and `cargo fmt --check` pass.

## Explicitly deferred

- External mission files, serialization, and mission editor support.
- Voice-over, animated cinematics, save/resume mid-tutorial.
- A generic visual scripting language.
- Full command-tree screen; selected-unit arrows and tutorial overlays are sufficient for NOW.
- Epistemically delayed succession. The tutorial can later be upgraded once units model discovery/confirmation of leader loss.
