use crate::actions::*;
use crate::units::*;
use bevy::camera::visibility::Visibility;
use bevy::prelude::*;
use std::collections::HashMap;

// ============================================================================
// SCENE ROOT MARKER
// ============================================================================

#[derive(Component)]
pub struct MissionScreenRoot;

// ============================================================================
// MENU SYSTEM
// ============================================================================

/// Resource tracking menu states
#[derive(Resource)]
pub struct MenuState {
    states: HashMap<MenuId, bool>,
}

impl MenuState {
    pub fn new() -> Self {
        let mut states = HashMap::new();
        states.insert(MenuId::Meta, true);
        states.insert(MenuId::Unit, false);
        states.insert(MenuId::Settings, false);

        Self { states }
    }

    pub fn is_open(&self, id: MenuId) -> bool {
        *self.states.get(&id).unwrap_or(&false)
    }

    pub fn toggle(&mut self, id: MenuId) {
        if let Some(state) = self.states.get_mut(&id) {
            *state = !*state;
        }
    }

    pub fn open(&mut self, id: MenuId) {
        self.states.insert(id, true);
    }

    pub fn close(&mut self, id: MenuId) {
        self.states.insert(id, false);
    }
}

/// Menu marker component
#[derive(Component)]
pub struct Menu {
    pub id: MenuId,
}

/// Possible menus within the mission screen
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuId {
    Meta,
    Unit,
    Settings,
}

// ============================================================================
// SYSTEMS
// ============================================================================

/// System to update menu visibility based on MenuState
pub fn update_menu_visibility(
    menu_state: Res<MenuState>,
    mut query: Query<(&Menu, &mut Visibility)>,
) {
    if !menu_state.is_changed() {
        return;
    }

    for (menu, mut visibility) in &mut query {
        *visibility = if menu_state.is_open(menu.id) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Left sidebar providing buttons for all other menus
pub fn setup_sidebar(mut commands: Commands) {
    commands
        .spawn((
            MissionScreenRoot,
            Menu { id: MenuId::Meta },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(200.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
        ))
        .with_children(|parent| {
            spawn_button(
                parent,
                ButtonConfig {
                    label: "U".to_string(),
                    action: ClickAction::ToggleMenu(MenuId::Unit),
                    width: 180.0,
                    height: 50.0,
                    ..default()
                },
            );

            spawn_button(
                parent,
                ButtonConfig {
                    label: "S".to_string(),
                    action: ClickAction::ToggleMenu(MenuId::Settings),
                    width: 180.0,
                    height: 50.0,
                    ..default()
                },
            );
        });
}

/// Bottom bar with unit spawning buttons
pub fn setup_unit_bar(mut commands: Commands) {
    commands
        .spawn((
            MissionScreenRoot,
            Menu { id: MenuId::Unit },
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(100.0),
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::FlexEnd,
                column_gap: Val::Px(10.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            spawn_button(
                parent,
                ButtonConfig {
                    label: "Spawn Private".to_string(),
                    action: ClickAction::SpawnSoldier {
                        rank: Rank::Private,
                        role: Role::Rifleman,
                        side: Side::Blue,
                    },
                    ..default()
                },
            );

            spawn_button(
                parent,
                ButtonConfig {
                    label: "Spawn Sergeant".to_string(),
                    action: ClickAction::SpawnSoldier {
                        rank: Rank::Sergeant,
                        role: Role::Rifleman,
                        side: Side::Blue,
                    },
                    ..default()
                },
            );

            spawn_button(
                parent,
                ButtonConfig {
                    label: "Spawn Medic".to_string(),
                    action: ClickAction::SpawnSoldier {
                        rank: Rank::Private,
                        role: Role::Medic,
                        side: Side::Blue,
                    },
                    ..default()
                },
            );
        });
}

/// Cleanup mission scene on exit
pub fn cleanup_mission_scene(mut commands: Commands, query: Query<Entity, With<MissionScreenRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

/// Spawn a soldier entity (gameplay logic)
pub fn spawn_soldier(commands: &mut Commands, rank: Rank, role: Role, side: Side) {
    commands.spawn((
        Soldier { rank, role },
        Allegiance { side },
        Health {
            current: 100,
            max: 100,
        },
        Mobility { speed: 10 },
        Inventory { items: vec![] },
    ));

    info!(
        "Soldier spawned! Rank: {:?}, Role: {:?}, Side: {:?}",
        rank, role, side
    );
}
