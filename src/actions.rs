#![allow(dead_code)] // allow temporarily while sketching
use crate::scenes::mission::MenuId;
use crate::units::*;
use bevy::prelude::*;

// ============================================================================
// CLICK ACTION SYSTEM
// ============================================================================

#[derive(Component, Clone)]
pub enum ClickAction {
    // Unit spawning
    SpawnSoldier { rank: Rank, role: Role, side: Side },

    // Entity interaction
    SelectUnit,
    SelectBuilding,

    // UI actions
    ToggleMenu(MenuId),
    OpenMenu(MenuId),
    CloseMenu(MenuId),

    // Future-proof
    Custom(String), // For prototyping
}

// ============================================================================
// BUTTON FACTORY
// ============================================================================

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

pub fn spawn_button(parent: &mut ChildSpawnerCommands, config: ButtonConfig) {
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

// ============================================================================
// INTERACTION SYSTEM
// ============================================================================

pub fn interaction_system(
    mut commands: Commands,
    mut menu_state: ResMut<crate::scenes::mission::MenuState>,
    mut query: Query<(&Interaction, &ClickAction, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, action, mut color) in &mut query {
        match *interaction {
            Interaction::Pressed => {
                handle_action(&mut commands, action, &mut menu_state);
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

fn handle_action(
    commands: &mut Commands,
    action: &ClickAction,
    menu_state: &mut crate::scenes::mission::MenuState,
) {
    match action {
        ClickAction::SpawnSoldier { rank, role, side } => {
            crate::scenes::mission::spawn_soldier(commands, *rank, *role, *side);
        }
        ClickAction::SelectUnit => {
            // Future: unit selection logic
            info!("Select unit clicked (not implemented yet)");
        }
        ClickAction::SelectBuilding => {
            // Future: building selection logic
            info!("Select building clicked (not implemented yet)");
        }
        ClickAction::ToggleMenu(menu_id) => menu_state.toggle(*menu_id),
        ClickAction::OpenMenu(menu_id) => menu_state.open(*menu_id),
        ClickAction::CloseMenu(menu_id) => menu_state.close(*menu_id),
        ClickAction::Custom(msg) => {
            info!("Custom action: {}", msg);
        }
    }
}

