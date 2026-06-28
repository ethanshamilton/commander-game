use crate::actions::*;
use crate::ai::perception::{EyeHeight, PerceptionMemory, SensorSignature, VisualSensor};
use crate::gameplay::comms::{CommsLinks, VoiceComms};
use crate::gameplay::components::{BattlefieldPosition, Heading};
use crate::gameplay::diagnostics::SimulationPerf;
use crate::gameplay::rendering::BattlefieldMap;
use crate::gameplay::simulation::SimulationClock;
use crate::missions::{DEMO_MISSION, MissionDefinition};
use crate::player::selection::{INFO_PANEL_WIDTH_PX, SelectedUnit};
use crate::units::*;
use bevy::camera::visibility::Visibility;
use bevy::prelude::*;
use std::collections::HashMap;

// ============================================================================
// SCENE ROOT MARKER
// ============================================================================

#[derive(Component)]
pub struct MissionScreenRoot;

#[derive(Component)]
pub struct MissionEntity;

#[derive(Component)]
pub struct SelectedUnitInfoPanel;

#[derive(Component)]
pub struct SelectedUnitInfoText;

#[derive(Component)]
pub struct SimulationClockText;

#[derive(Component)]
pub struct SimulationPerfText;

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
            .add_systems(
                OnEnter(crate::GameState::MissionScreen),
                (setup_mission_ui, setup_demo_mission),
            )
            .add_systems(
                OnExit(crate::GameState::MissionScreen),
                cleanup_mission_scene,
            )
            .add_systems(
                Update,
                (
                    update_menu_visibility,
                    update_selected_unit_info_panel,
                    update_simulation_clock_text,
                    update_simulation_perf_text,
                )
                    .run_if(in_state(crate::GameState::MissionScreen)),
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

pub fn update_selected_unit_info_panel(
    selected: Res<SelectedUnit>,
    mut panel_query: Query<&mut Node, With<SelectedUnitInfoPanel>>,
    mut text_query: Query<&mut Text, With<SelectedUnitInfoText>>,
    units: Query<(
        &Soldier,
        &Allegiance,
        &Health,
        &Mobility,
        &BattlefieldPosition,
        Option<&Heading>,
        Option<&VisualSensor>,
        Option<&PerceptionMemory>,
    )>,
) {
    let Ok(mut panel_node) = panel_query.single_mut() else {
        return;
    };

    let Some(entity) = selected.entity else {
        panel_node.display = Display::None;
        return;
    };

    let Ok((soldier, allegiance, health, mobility, position, heading, visual_sensor, memory)) =
        units.get(entity)
    else {
        panel_node.display = Display::None;
        return;
    };

    panel_node.display = Display::Flex;

    let position_m = position.0;
    let heading_text = heading
        .map(|Heading(angle)| format!("{angle:.2} rad"))
        .unwrap_or_else(|| "n/a".to_string());
    let sensor_text = visual_sensor
        .map(|sensor| {
            format!(
                "Visual range: {:.0}m\nVisual FOV: {:.0}°",
                sensor.range_m,
                sensor.fov_radians.to_degrees()
            )
        })
        .unwrap_or_else(|| "Visual sensor: none".to_string());
    let contact_count = memory.map(|memory| memory.contacts.len()).unwrap_or(0);

    let Ok(mut text) = text_query.single_mut() else {
        return;
    };

    **text = format!(
        "Side: {:?}\nRank: {:?}\nRole: {:?}\n\nHealth: {}/{}\nSpeed: {}\n\nPosition: ({:.1}m, {:.1}m)\nHeading: {}\n\n{}\nContacts: {}",
        allegiance.side,
        soldier.rank,
        soldier.role,
        health.current,
        health.max,
        mobility.speed,
        position_m.x,
        position_m.y,
        heading_text,
        sensor_text,
        contact_count,
    );
}

pub fn update_simulation_clock_text(
    clock: Res<SimulationClock>,
    mut text_query: Query<&mut Text, With<SimulationClockText>>,
) {
    if !clock.is_changed() {
        return;
    }

    let Ok(mut text) = text_query.single_mut() else {
        return;
    };

    let minutes = (clock.elapsed_s / 60.0).floor() as u32;
    let seconds = (clock.elapsed_s % 60.0).floor() as u32;
    let paused = if clock.paused { " PAUSED" } else { "" };
    **text = format!("T+{minutes:02}:{seconds:02}  tick {}{paused}", clock.tick);
}

pub fn update_simulation_perf_text(
    perf: Res<SimulationPerf>,
    mut text_query: Query<(&mut Text, &mut TextColor), With<SimulationPerfText>>,
) {
    if !perf.is_changed() {
        return;
    }

    let Ok((mut text, mut text_color)) = text_query.single_mut() else {
        return;
    };

    let utilization_percent = perf.utilization * 100.0;
    let filled = (perf.utilization * 10.0).round().clamp(0.0, 10.0) as usize;
    let meter = format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled));

    **text = format!(
        "SIM {:04.1}ms / {:04.1}ms [{}] {:03.0}%",
        perf.last_tick_s * 1000.0,
        perf.tick_budget_s * 1000.0,
        meter,
        utilization_percent,
    );

    *text_color = TextColor(if perf.utilization >= 1.0 {
        Color::srgb(1.0, 0.1, 0.1)
    } else if perf.utilization >= 0.8 {
        Color::srgb(1.0, 0.55, 0.0)
    } else if perf.utilization >= 0.5 {
        Color::srgb(1.0, 0.9, 0.0)
    } else {
        Color::srgb(0.7, 1.0, 0.7)
    });
}

/// Setup the entire mission UI hierarchy using flexbox
pub fn setup_mission_ui(mut commands: Commands) {
    commands
        .spawn((
            MissionScreenRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                right: Val::Px(12.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::Px(4.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.02, 0.75)),
        ))
        .with_children(|parent| {
            parent.spawn((
                SimulationClockText,
                Text::new("T+00:00  tick 0"),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.7, 1.0, 0.7)),
            ));

            parent.spawn((
                SimulationPerfText,
                Text::new("SIM 00.0ms / 50.0ms [░░░░░░░░░░] 000%"),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::srgb(0.7, 1.0, 0.7)),
            ));
        });

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

            // Right-side selected unit info panel. `Display::None` keeps it out of layout
            // until a unit is selected.
            parent
                .spawn((
                    SelectedUnitInfoPanel,
                    Node {
                        display: Display::None,
                        width: Val::Px(INFO_PANEL_WIDTH_PX),
                        height: Val::Auto,
                        margin: UiRect::top(Val::Px(76.0)),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(12.0)),
                        row_gap: Val::Px(8.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.08, 0.08, 0.08)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("Selected Unit"),
                        TextFont {
                            font_size: FontSize::Px(22.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));

                    panel.spawn((
                        SelectedUnitInfoText,
                        Text::new(""),
                        TextFont {
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.85, 0.85, 0.85)),
                    ));
                });
        });
}

/// Cleanup mission scene on exit
pub fn cleanup_mission_scene(
    mut commands: Commands,
    ui_roots: Query<Entity, With<MissionScreenRoot>>,
    mission_entities: Query<Entity, With<MissionEntity>>,
) {
    for entity in &ui_roots {
        commands.entity(entity).despawn();
    }

    for entity in &mission_entities {
        commands.entity(entity).despawn();
    }
}

pub fn setup_demo_mission(mut commands: Commands) {
    spawn_mission(&mut commands, &DEMO_MISSION);
}

pub fn spawn_mission(commands: &mut Commands, mission: &MissionDefinition) {
    info!(
        "Spawning mission: {} on map: {}",
        mission.name, mission.map.name
    );
    commands.insert_resource(BattlefieldMap::from_definition(mission.map));

    for unit in mission.units {
        spawn_soldier_at(
            commands,
            unit.rank,
            unit.role,
            unit.side,
            Vec2::new(unit.position_meters[0], unit.position_meters[1]),
            unit.heading_radians,
        );
    }
}

/// Spawn a soldier entity (gameplay logic)
pub fn spawn_soldier(commands: &mut Commands, rank: Rank, role: Role, side: Side) {
    spawn_soldier_at(commands, rank, role, side, Vec2::ZERO, 0.0);
}

pub fn spawn_soldier_at(
    commands: &mut Commands,
    rank: Rank,
    role: Role,
    side: Side,
    position: Vec2,
    heading_radians: f32,
) {
    commands.spawn((
        MissionEntity,
        Soldier { rank, role },
        Allegiance { side },
        Health {
            current: 100,
            max: 100,
        },
        Mobility { speed: 1 },
        Inventory { items: vec![] },
        BattlefieldPosition(position),
        Heading(heading_radians),
        VisualSensor::default(),
        EyeHeight::default(),
        SensorSignature::default(),
        PerceptionMemory::default(),
        VoiceComms::default(),
        CommsLinks::default(),
    ));

    info!(
        "Soldier spawned! Rank: {:?}, Role: {:?}, Side: {:?}",
        rank, role, side
    );
}
