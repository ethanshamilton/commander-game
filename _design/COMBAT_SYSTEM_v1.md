# Combat System v1

## Goal

Implement combat as **probabilistic resolution with visual ballistics**.

The simulation decides whether a shot hits using a configurable probability model. The ballistic/tracer visualization is then spawned as a visual receipt of that already-resolved shot. The projectile visual should not determine combat truth.

```text
combat sim: shooter + target + context -> P(hit) -> roll -> hit/miss/effects
visuals:    resolved shot -> moving tracer along chosen shot line
```

## Core Principles

1. **Simulation first, visualization second**
   - Hit/miss is resolved immediately by the combat system.
   - Tracers are visual-only and do not perform physical collision.

2. **Explicit targeting first**
   - v1 should not implement autonomous target selection.
   - Units fire because they have been given a target/fire command.
   - Autonomous target selection is an AI feature for later.

3. **Individual-level execution**
   - Commands are issued to individual units.
   - A unit only fires if it can locally perceive and engage its assigned target.

4. **Separate movement and combat orders**
   - A unit should eventually be able to move and fire at the same time.
   - Do not overload the existing movement `UnitOrder` with combat semantics.

## New Components

### Weapon

```rust
#[derive(Component)]
pub struct Weapon {
    pub max_range_m: f32,
    pub effective_range_m: f32,
    pub damage: i32,
    pub base_accuracy: f32,
    pub cooldown_ticks: u64,
    pub projectile_speed_mps: f32,
    pub tracer_length_m: f32,
}
```

Represents a unit's currently equipped weapon. For v1, spawned soldiers can receive a default rifle.

### CombatState

```rust
#[derive(Component, Default)]
pub struct CombatState {
    pub next_fire_tick: u64,
}
```

Tracks weapon cooldown / fire timing.

### Marksmanship / SoldierSkill

```rust
#[derive(Component)]
pub struct Marksmanship {
    pub value: f32,
}
```

or more generally:

```rust
#[derive(Component)]
pub struct SoldierSkill {
    pub marksmanship: f32,
}
```

Used as one factor in hit probability. Exact naming TBD.

### CombatOrder

```rust
#[derive(Component, Debug, Clone, Copy)]
pub enum CombatOrder {
    FireAt { target: Entity },
    HoldFire,
}
```

This should be separate from the existing movement-oriented `UnitOrder`.

A unit can therefore have both:

```text
UnitOrder::MoveTo(...)
CombatOrder::FireAt { target }
```

## Targeting v1

Start with explicit target commands only.

Suggested UI semantics:

```text
left-click friendly unit      -> select unit
right-click terrain           -> issue movement order
right-click known hostile     -> issue CombatOrder::FireAt { target }
```

Issuing a fire command should use the same command mediation stack as movement:

```text
selected unit is friendly
selected unit knowledge is current
selected unit is reachable through comms from PlayerControlledUnit
PlayerControlledUnit can command selected unit through CommandForest
target hostile is known/observed enough
```

Initial target visibility rule:

```text
fire command can target hostile units actively observed this tick
```

Later extensions can support stale contacts, suppressive fire at last-known position, ambushes, etc.

## Combat Execution v1

A combat system should run in `SimulationSet::Combat`.

For each soldier with:

```text
Weapon
CombatState
CombatOrder::FireAt { target }
PerceptionMemory
BattlefieldPosition
```

perform:

1. Check weapon cooldown:

```text
clock.tick >= combat_state.next_fire_tick
```

2. Check local perception:

```text
target appears in shooter's current-tick hostile visual contacts
```

The command supplies intent, but local perception gates execution.

3. Check target is valid and in range.

4. Compute hit probability.

5. Roll hit/miss.

6. If hit, apply damage to target health.

7. Spawn visual tracer using resolved outcome.

8. Set next fire tick.

Pseudo:

```rust
for shooter in shooters {
    let CombatOrder::FireAt { target } = combat_order else { continue; };
    if clock.tick < combat_state.next_fire_tick { continue; }
    if !memory.has_current_hostile_visual_contact(target, clock.tick) { continue; }

    let distance_m = shooter_position.distance(target_position);
    if distance_m > weapon.max_range_m { continue; }

    let p_hit = compute_hit_probability(...);
    let roll = rng.random::<f32>();
    let hit = roll <= p_hit;

    let impact_position_m = if hit {
        target_position
    } else {
        random_miss_endpoint(shooter_position, target_position, p_hit, roll)
    };

    if hit {
        health.current -= weapon.damage;
    }

    spawn_tracer(shooter_position, impact_position_m, weapon, hit);
    combat_state.next_fire_tick = clock.tick + weapon.cooldown_ticks;
}
```

## Hit Probability Model

Start simple, but make it extensible.

Example:

```rust
fn hit_probability(ctx: ShotContext) -> f32 {
    let range_factor = range_modifier(
        ctx.distance_m,
        ctx.weapon.effective_range_m,
        ctx.weapon.max_range_m,
    );

    let p = ctx.weapon.base_accuracy
        * range_factor
        * ctx.marksmanship
        * ctx.contact_confidence;

    p.clamp(0.01, 0.95)
}
```

Potential factors for later:

- weapon base accuracy
- range
- shooter marksmanship
- shooter suppression
- shooter fatigue
- shooter wounds
- target movement
- target stance
- target exposure / cover
- lighting / weather
- perception/contact confidence
- aiming time
- fire mode
- morale
- terrain

## Miss Endpoint Selection

Misses do not need exact physical modeling.

For visual plausibility:

```text
line = shooter -> target
miss endpoint = target position + forward overshoot + lateral jitter
```

Miss severity can be based on how badly the shot missed:

```rust
let miss_severity = ((roll - p_hit) / (1.0 - p_hit)).clamp(0.0, 1.0);
```

Then:

```rust
let forward = (target - shooter).normalize();
let lateral = forward.perp();
let overshoot_m = rng.range(0.0..10.0) * miss_severity;
let lateral_m = rng.range(-8.0..8.0) * miss_severity;
let impact = target + forward * overshoot_m + lateral * lateral_m;
```

A barely missed shot lands near the target. A wild miss lands farther away.

## Visual Ballistics / Tracers

Spawn visual tracer entities after combat resolution.

```rust
#[derive(Component)]
pub struct Tracer {
    pub start_m: Vec2,
    pub end_m: Vec2,
    pub speed_mps: f32,
    pub length_m: f32,
    pub elapsed_s: f32,
    pub hit: bool,
}
```

The tracer travels along the resolved line over time.

Each `Update` frame:

```rust
elapsed_s += time.delta_secs();
distance_m = start_m.distance(end_m);
travelled_m = elapsed_s * speed_mps;
tip_t = (travelled_m / distance_m).clamp(0.0, 1.0);
tail_t = ((travelled_m - length_m) / distance_m).clamp(0.0, 1.0);

tail = start_m.lerp(end_m, tail_t);
tip = start_m.lerp(end_m, tip_t);
gizmos.line_2d(tail.map(meters), tip.map(meters), color);
```

When `tip_t >= 1.0`, despawn the tracer.

Possible colors:

```text
hit:  bright yellow/white
miss: dimmer orange/yellow
```

The tracer should start from the border of the shooter's unit circle. For hits, it should probably stop at the target circle border. For misses, it should continue to the randomized miss endpoint.

## Events vs Direct Spawn

Two implementation options:

### Option A: Combat directly spawns `Tracer`

Simpler for v1.

```rust
commands.spawn(Tracer { ... });
```

### Option B: Emit `ShotResolved` event

Cleaner long-term.

```rust
#[derive(Event)]
pub struct ShotResolved {
    pub shooter: Entity,
    pub target: Entity,
    pub shooter_position_m: Vec2,
    pub target_position_m: Vec2,
    pub impact_position_m: Vec2,
    pub hit: bool,
    pub damage: i32,
}
```

Then another system consumes the event and spawns tracer visuals.

Long-term, events are better for observability, audio, effects, debugging, and replay. For v1, direct tracer spawn is acceptable if we want less boilerplate.

## RNG

Use deterministic seeded RNG rather than thread-local randomness, so combat is reproducible.

Possible resource:

```rust
#[derive(Resource)]
pub struct CombatRng(pub StdRng);
```

This likely requires adding `rand` to `Cargo.toml`.

## Future Extensions

### Combat Orders

```rust
CombatOrder::SuppressArea { center_m: Vec2, radius_m: f32 }
CombatOrder::FireAtLastKnown { target: Entity, position_m: Vec2 }
CombatOrder::Ambush { trigger_area_m: ..., target_side: Side }
```

### Rules of Engagement

```rust
pub struct RulesOfEngagement {
    pub fire_without_command: bool,
    pub return_fire: bool,
    pub prioritize_closest: bool,
}
```

### Suppression

Shots can apply suppression even when they miss.

```rust
#[derive(Component)]
pub struct Suppression {
    pub value: f32,
}
```

Suppression can later affect:

- accuracy
- movement
- command responsiveness
- morale
- perception/reporting

### Health States

Eventually replace plain HP-only consequences with:

```text
healthy
wounded
incapacitated
killed
```

## Suggested Implementation Order

1. Create `src/gameplay/combat.rs` and `docs/gameplay/combat.md`.
2. Add `CombatPlugin` to `GameplayPlugins`.
3. Add `Weapon`, `CombatState`, `Marksmanship`/`SoldierSkill`, `CombatOrder`.
4. Add default rifle/combat components in `spawn_soldier_at`.
5. Add right-click hostile targeting to issue `CombatOrder::FireAt { target }`.
6. Add combat resolution in `SimulationSet::Combat`.
7. Add visual `Tracer` component and rendering/cleanup systems.
8. Add deterministic `CombatRng`.

## Open Questions

- Should `CombatOrder::FireAt` persist until explicitly changed, or clear when target is no longer visible?
- Should a unit be able to fire while moving in v1, or should movement reduce accuracy heavily?
- Should hit tracers terminate at target center or target circle border?
- Should misses continue past the target by default?
- Should firing reveal shooter position through sound/perception signature effects?
