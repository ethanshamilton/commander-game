#![allow(dead_code)] // allow temporarily while sketching

use crate::screens::mission;
use crate::screens::mission::{MenuId, MenuState};
use crate::units::*;
use bevy::prelude::*;
use bevy::ui::Checked;
use bevy::ui_widgets::{
    Activate, Button, Checkbox, ValueChange, checkbox_self_update, observe,
};

// ============================================================================
// UI ACTIONS
// ============================================================================

/// Domain-level intent attached to an activated UI control.
///
/// Bevy's widget components decide *when* a control is activated; this enum decides
/// *what the game should do* after that activation.
#[derive(Component, Clone)]
pub enum ClickAction {
    // Unit spawning
    SpawnSoldier { rank: Rank, role: Role, side: Side },

    // Entity interaction
    SelectUnit,
    SelectBuilding,

    // UI actions
    OpenMenu(MenuId),
    CloseMenu(MenuId),

    // Future-proof
    Custom(String),
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
            observe(handle_button_activate),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(config.label),
                TextFont {
                    font_size: FontSize::Px(config.text_size.unwrap_or(20.0)),
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));
        });
}

pub struct MenuToggleConfig {
    pub label: String,
    pub menu_id: MenuId,
    pub checked: bool,
    pub width: f32,
    pub height: f32,
}

impl Default for MenuToggleConfig {
    fn default() -> Self {
        Self {
            label: "Toggle".to_string(),
            menu_id: MenuId::Unit,
            checked: false,
            width: 180.0,
            height: 50.0,
        }
    }
}

pub fn spawn_menu_toggle(parent: &mut ChildSpawnerCommands, config: MenuToggleConfig) {
    let checked = config.checked;

    let mut entity = parent.spawn((
        Checkbox,
        MenuToggle { id: config.menu_id },
        Node {
            width: Val::Px(config.width),
            height: Val::Px(config.height),
            border: UiRect::all(Val::Px(5.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BorderColor::all(Color::BLACK),
        BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
        observe(checkbox_self_update),
        observe(handle_menu_toggle_change),
    ));

    if checked {
        entity.insert(Checked);
    }

    entity.with_children(|parent| {
        parent.spawn((
            Text::new(config.label),
            TextFont {
                font_size: FontSize::Px(20.0),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.9, 0.9)),
        ));
    });
}

#[derive(Component)]
pub struct MenuToggle {
    pub id: MenuId,
}

// ============================================================================
// ACTION OBSERVERS
// ============================================================================

fn handle_button_activate(
    activate: On<Activate>,
    mut commands: Commands,
    mut menu_state: ResMut<MenuState>,
    actions: Query<&ClickAction>,
) {
    if let Ok(action) = actions.get(activate.entity) {
        handle_action(&mut commands, action, &mut menu_state);
    }
}

fn handle_menu_toggle_change(
    value_change: On<ValueChange<bool>>,
    mut menu_state: ResMut<MenuState>,
    toggles: Query<&MenuToggle>,
) {
    if let Ok(toggle) = toggles.get(value_change.source) {
        menu_state.set(toggle.id, value_change.value);
    }
}

fn handle_action(commands: &mut Commands, action: &ClickAction, menu_state: &mut MenuState) {
    match action {
        ClickAction::SpawnSoldier { rank, role, side } => {
            mission::spawn_soldier(commands, *rank, *role, *side);
        }
        ClickAction::SelectUnit => {
            info!("Select unit clicked (not implemented yet)");
        }
        ClickAction::SelectBuilding => {
            info!("Select building clicked (not implemented yet)");
        }
        ClickAction::OpenMenu(menu_id) => {
            menu_state.open(*menu_id);
        }
        ClickAction::CloseMenu(menu_id) => {
            menu_state.close(*menu_id);
        }
        ClickAction::Custom(msg) => {
            info!("Custom action: {}", msg);
        }
    }
}
