use crate::actions::*;
use crate::gameplay::components::{BattlefieldPosition, Heading};
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

    pub fn set(&mut self, id: MenuId, is_open: bool) {
        self.states.insert(id, is_open);
    }

    pub fn open(&mut self, id: MenuId) {
        self.set(id, true);
    }

    pub fn close(&mut self, id: MenuId) {
        self.set(id, false);
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
// PLUGIN
// ============================================================================

pub struct MissionScreenPlugin;

impl Plugin for MissionScreenPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MenuState::new())
            .add_systems(OnEnter(crate::GameState::MissionScreen), setup_mission_ui)
            .add_systems(OnExit(crate::GameState::MissionScreen), cleanup_mission_scene)
            .add_systems(
                Update,
                update_menu_visibility.run_if(in_state(crate::GameState::MissionScreen)),
            );
    }
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

/// Setup the entire mission UI hierarchy using flexbox
pub fn setup_mission_ui(mut commands: Commands) {
    // Root flex container (fills screen, horizontal layout)
    commands
        .spawn((
            MissionScreenRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
        ))
        .with_children(|parent| {
            // Left sidebar (fixed width, always visible)
            parent
                .spawn((
                    Menu { id: MenuId::Meta },
                    Node {
                        width: Val::Px(200.0),
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(10.0)),
                        row_gap: Val::Px(10.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
                ))
                .with_children(|sidebar| {
                    spawn_menu_toggle(
                        sidebar,
                        MenuToggleConfig {
                            label: "U".to_string(),
                            menu_id: MenuId::Unit,
                            checked: false,
                            width: 180.0,
                            height: 50.0,
                        },
                    );

                    spawn_menu_toggle(
                        sidebar,
                        MenuToggleConfig {
                            label: "S".to_string(),
                            menu_id: MenuId::Settings,
                            checked: false,
                            width: 180.0,
                            height: 50.0,
                        },
                    );
                });

            // Main content area (flex to fill remaining space, vertical layout)
            parent
                .spawn(Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|main_area| {
                    // Content area (grows to push unit bar to bottom)
                    main_area.spawn(Node {
                        flex_grow: 1.0,
                        ..default()
                    });

                    // Unit bar at bottom (fixed height, toggleable)
                    main_area
                        .spawn((
                            Menu { id: MenuId::Unit },
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(100.0),
                                justify_content: JustifyContent::FlexStart,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(10.0),
                                padding: UiRect::all(Val::Px(10.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                        ))
                        .with_children(|unit_bar| {
                            spawn_button(
                                unit_bar,
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
                                unit_bar,
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
                                unit_bar,
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
                });
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
        BattlefieldPosition(Vec2::ZERO),
        Heading(0.0),
    ));

    info!(
        "Soldier spawned! Rank: {:?}, Role: {:?}, Side: {:?}",
        rank, role, side
    );
}

