# Generic Clickable System + Button Factory

## Overview
Create a flexible interaction system that works for both UI buttons and world entities (units, buildings, etc.), plus helper functions to easily spawn buttons.

## Design Approach

### 1. Generic Clickable Component
```rust
#[derive(Component, Clone)]
pub enum ClickAction {
    // Unit spawning
    SpawnSoldier { rank: Rank, role: Role, side: Side },

    // Entity interaction
    SelectUnit,
    SelectBuilding,

    // UI actions
    OpenMenu(MenuType),
    CloseMenu,

    // Future-proof
    Custom(String), // For prototyping
}
```

This enum-based approach:
- Works for both UI and world entities
- Type-safe and exhaustive
- Can be split into separate systems later if needed (UIAction, WorldAction, etc.)

### 2. Unified Interaction System
Replace the current `button_system` with a more generic system:

```rust
fn interaction_system(
    mut commands: Commands,
    mut query: Query<(&Interaction, &ClickAction, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, action, mut color) in &mut query {
        match *interaction {
            Interaction::Pressed => {
                handle_action(&mut commands, action);
                // Visual feedback
                *color = BackgroundColor(Color::srgb(0.35, 0.75, 0.35));
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgb(0.25, 0.25, 0.25));
            }
            Interaction::None => {
                *color = BackgroundColor(Color::srgb(0.15, 0.15, 0.15));
            }
        }
    }
}

fn handle_action(commands: &mut Commands, action: &ClickAction) {
    match action {
        ClickAction::SpawnSoldier { rank, role, side } => {
            spawn_soldier(commands, *rank, *role, *side);
        }
        ClickAction::SelectUnit => {
            // Future: unit selection logic
        }
        // ... other actions
    }
}
```

### 3. Button Factory Helper
```rust
pub struct ButtonConfig {
    pub label: String,
    pub action: ClickAction,
    pub width: f32,
    pub height: f32,
    // Optional style overrides
    pub bg_color: Option<Color>,
    pub text_size: Option<f32>,
}

impl Default for ButtonConfig {
    fn default() -> Self {
        Self {
            label: "Button".to_string(),
            action: ClickAction::Custom("".to_string()),
            width: 150.0,
            height: 65.0,
            bg_color: None,
            text_size: None,
        }
    }
}

fn spawn_button(parent: &mut ChildBuilder, config: ButtonConfig) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(config.width),
                height: Val::Px(config.height),
                border: UiRect::all(Val::Px(5.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(Color::BLACK),
            BackgroundColor(config.bg_color.unwrap_or(Color::srgb(0.15, 0.15, 0.15))),
            config.action,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(config.label),
                TextFont {
                    font_size: config.text_size.unwrap_or(20.0),
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));
        });
}
```

### 4. Updated setup_ui Example
```rust
fn setup_ui(mut commands: Commands) {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|parent| {
            // Spawn multiple buttons easily
            spawn_button(parent, ButtonConfig {
                label: "Spawn Private".to_string(),
                action: ClickAction::SpawnSoldier {
                    rank: Rank::Private,
                    role: Role::Rifleman,
                    side: Side::Blue,
                },
                ..default()
            });

            spawn_button(parent, ButtonConfig {
                label: "Spawn Sergeant".to_string(),
                action: ClickAction::SpawnSoldier {
                    rank: Rank::Sergeant,
                    role: Role::Rifleman,
                    side: Side::Blue,
                },
                ..default()
            });
        });
}
```

## Implementation Steps

1. **Create action enum**: Add `ClickAction` enum to `components.rs`
2. **Create button factory**: Add `ButtonConfig` struct and `spawn_button` helper function
3. **Replace button system**: Update `button_system` to be `interaction_system` with generic action handling
4. **Update setup_ui**: Use new factory to spawn multiple buttons
5. **Remove old code**: Delete `SpawnSoldierButton` marker component

## Files to Modify
- `src/components.rs` - Add `ClickAction` enum and `ButtonConfig` struct
- `src/main.rs` - Replace button system, add factory function, update setup_ui

## Future Evolution
- **Icons/Images**: Extend `ButtonConfig` to support icon sprites
- **Layout System**: Add positioning/grid system for button panels
- **World Entities**: Add `ClickAction` to soldiers/buildings with mouse picking
- **Separate Systems**: If UI vs world interactions diverge significantly, split `ClickAction` into `UIAction` and `WorldAction` enums

## Trade-offs
**Pros:**
- Single interaction system for all clickables
- Easy to add new button types
- Factory reduces boilerplate
- Flexible enough to adapt as needs evolve

**Cons:**
- All actions in one enum (might grow large - can be split later)
- BackgroundColor changes only work for UI buttons (world entities need different feedback)
