# AGENTS.md

Basic repo map for coding agents.

## Project

Rust/Bevy game prototype. Uses Bevy `0.19`.

Core idea: simulation-first tactical command game. Gameplay display is currently radar/vector-style via Bevy `Gizmos`; UI is Bevy UI/widgets.

## Structure

```txt
src/
  main.rs              # App setup: DefaultPlugins, GameplayPlugins, ScreensPlugins, GameState
  actions.rs           # UI action components + widget factories + button/toggle observers
  units.rs             # Soldier/unit ECS components and enums

  screens/             # App-state-level screens, not Bevy scenes
    mod.rs             # ScreensPlugins plugin group + shared screen camera setup
    mission.rs         # Mission screen UI layout, menu state, spawn_soldier
    main_menu.rs       # Stub screen plugin
    settings.rs        # Stub screen plugin

  gameplay/
    mod.rs             # GameplayPlugins plugin group
    components.rs      # BattlefieldPosition, Heading
    simulation.rs      # Stub SimulationPlugin
    rendering.rs       # Gizmo-based radar/grid/unit rendering
```

## Vocabulary

- `screens/` = game/app modes like mission, main menu, settings.
- Bevy `Scene`/BSN = reusable entity composition/prefab-like object. Avoid using “scene” for app screens.
- Gameplay rendering is immediate-mode and derived from simulation state.

## Current flow

- Default state is `GameState::MissionScreen`.
- `ScreensPlugins` registers screen lifecycle systems.
- `GameplayPlugins` registers simulation/rendering systems.
- Mission UI has side/bottom bars over a full-screen gameplay grid.
- Buttons use `bevy::ui_widgets::Button` + `Activate` observers.
- Menu toggles use `bevy::ui_widgets::Checkbox` + `ValueChange<bool>`.

## Notes

- `DefaultPlugins` already includes `UiWidgetsPlugins` in Bevy 0.19; do not add `UiWidgetsPlugins` separately.
- `Gizmos` are immediate-mode: draw every frame in systems.
- Keep simulation and rendering separate: simulation owns truth, rendering reads it.
