# Scene-Based Architecture

## Overview
The game will have multiple distinct "scenes" (game modes/screens), each with its own UI layout, systems, and logic. This document outlines the architecture for managing these scenes.

## Scene Types

### Core Scenes
- **Main Menu**: Entry point, save game management, settings access
- **Mission Screen**: Core gameplay - unit control, building, combat
- **Tech Tree**: Research and progression management
- **Settings**: Game configuration

### Future Scenes (Potential)
- Briefing/debriefing screens
- Unit/building encyclopedias
- Multiplayer lobby
- Map editor

## Technical Approach: Bevy States

Use Bevy's built-in state management system:

```rust
#[derive(States, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    MainMenu,
    MissionScreen,
    TechTree,
    Settings,
}
```

### State Lifecycle
- **OnEnter(State)**: Spawn scene UI and initialize scene-specific resources
- **Update + run_if(in_state(State))**: Systems that only run in specific states
- **OnExit(State)**: Cleanup/despawn scene entities

### Scene Root Pattern
Each scene spawns a root entity with a marker component:

```rust
#[derive(Component)]
pub struct MainMenuRoot;

#[derive(Component)]
pub struct MissionScreenRoot;

pub fn setup_main_menu(mut commands: Commands) {
    commands
        .spawn((
            MainMenuRoot,
            Node { /* ... */ },
        ))
        .with_children(|parent| {
            // Scene UI hierarchy
        });
}

pub fn cleanup_scene<T: Component>(
    mut commands: Commands,
    query: Query<Entity, With<T>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
```

## Module Structure

### Phase 1: Simple (Start Here)
```
src/
  main.rs              // App, state setup, shared plugins
  units.rs             // Shared game data
  actions.rs           // Shared input/actions (if cross-scene)

  scenes/
    mod.rs             // Re-exports
    main_menu.rs       // Main menu UI + systems
    mission.rs         // Mission screen UI + gameplay
    tech_tree.rs       // Tech tree UI + research logic
    settings.rs        // Settings UI
```

**When to use**: Starting out, scenes < 500 lines each

### Phase 2: Complex Scenes (When Needed)
```
src/
  scenes/
    mod.rs
    main_menu.rs       // Still simple enough for one file

    mission/
      mod.rs           // Re-exports and scene setup
      ui.rs            // Sidebar, unit bar, menus
      gameplay.rs      // Unit spawning, combat systems
      camera.rs        // Camera controls
      resources.rs     // Mission-specific resources

    tech_tree/
      mod.rs
      ui.rs
      research.rs
```

**When to split**:
- Scene file > 300-500 lines
- Distinct subsystems within scene (UI vs gameplay vs camera)
- Multiple people working on same scene

## Scene-Specific Considerations

### Main Menu
**Layout**: Centered vertical stack of buttons
**Systems**: None (UI only)
**Transitions**:
- New Game → Mission Screen
- Settings → Settings Screen
- Exit → Close app

### Mission Screen
**Layout**: Flexbox with sidebar + main area
- Left sidebar: Menu of menus (200px fixed)
- Main area: Dynamic content
- Bottom: Unit spawn bar (toggleable)
- Other menus overlay or dock

**Systems**:
- Unit spawning/selection
- Building placement
- Combat/AI
- Camera controls
- Menu visibility management

**Menus** (within mission):
```rust
pub enum MissionMenu {
    UnitBar,
    BuildingPanel,
    CommandPanel,
    // etc.
}
```

**Root Structure**:
```rust
MissionScreenRoot
├─ Sidebar (Menu { id: MenuId::Sidebar })
│  └─ Menu toggle buttons
└─ MainArea
   ├─ UnitBar (Menu { id: MenuId::UnitBar })
   ├─ BuildingPanel (Menu { id: MenuId::BuildingPanel })
   └─ etc.
```

### Tech Tree
**Layout**: Full-screen canvas, possibly scrollable
**Systems**: Research progression, node unlocking
**Transitions**: Back button → Mission Screen or Main Menu

### Settings
**Layout**: Centered panel with sections
**Systems**: Settings persistence
**Transitions**: Back button → previous scene

## Implementation Plan

### Step 1: Create State Infrastructure
1. Add `#[derive(States)]` enum to main.rs or new states.rs
2. Add `.init_state::<GameState>()` to App
3. Test state transitions with minimal UI

### Step 2: Restructure Existing Code
1. Create `src/scenes/` directory
2. Create `scenes/mod.rs` with exports
3. Move current UI code → `scenes/mission.rs`
4. Update imports in main.rs
5. Wrap mission setup in `OnEnter(GameState::MissionScreen)`
6. Add cleanup with `OnExit(GameState::MissionScreen)`

### Step 3: Add Main Menu Stub
1. Create `scenes/main_menu.rs`
2. Simple centered "Start Mission" button
3. Button action: `NextState::set(GameState::MissionScreen)`
4. Start app in `GameState::MainMenu` instead of going straight to mission

### Step 4: Expand as Needed
1. Add tech tree, settings screens
2. Split mission.rs into mission/ directory if needed
3. Add transitions between all scenes

## Menu System Integration

The current `MenuId` enum is for **menus within the mission screen**, not game-wide scenes:

```rust
// For top-level scenes
pub enum GameState {
    MainMenu,
    MissionScreen,
    // ...
}

// For menus WITHIN mission screen
pub enum MenuId {
    Sidebar,
    UnitBar,
    BuildingPanel,
    // ...
}
```

These are separate concerns:
- **GameState**: Which scene/mode the game is in
- **MenuId**: Which UI panels are visible within a scene

The `MenuState` resource and visibility system stay within the mission screen.

## Benefits of This Architecture

1. **Separation of concerns**: Each scene is self-contained
2. **Performance**: Only relevant systems run in each state
3. **Clean transitions**: Bevy handles state changes, automatic cleanup
4. **Scalability**: Easy to add new scenes without touching existing code
5. **Team-friendly**: Different developers can own different scenes
6. **Flexible layouts**: Each scene can have completely different UI structure

## Anti-Patterns to Avoid

- ❌ Don't put all scenes in one file
- ❌ Don't run mission systems in main menu (use `.run_if(in_state(...))`)
- ❌ Don't manually despawn everything - use state transitions
- ❌ Don't share UI hierarchies between scenes - spawn fresh each time
- ❌ Don't conflate game states with menu visibility

## References

- Bevy States: https://bevyengine.org/learn/book/next-steps/states/
- Large Bevy projects often use this pattern (see bevy_jam entries)
