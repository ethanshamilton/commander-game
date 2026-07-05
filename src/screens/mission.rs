#![doc = include_str!("../../docs/screens/mission.md")]

use crate::actors::skills::Marksmanship;
use crate::actors::units::*;
use crate::actors::weapons::Weapon;
use crate::ai::htn::executor::{Autonomous, DomainId, DomainRef};
use crate::ai::htn::synthesis::PlannerBelief;
use crate::ai::htn::trace::{
    DecisionTrace, PlanRejectionReason, ReplanTrigger, TraceEvent, TraceRecord,
};
use crate::ai::perception::{
    AuditorySensor, EyeHeight, PerceptionMemory, SensorSignature, VisualSensor,
};
use crate::gameplay::combat::{CombatOrder, CombatState};
use crate::gameplay::command::{CommandForest, UnitIdentity};
use crate::gameplay::comms::{CommsLinks, VoiceComms};
use crate::gameplay::diagnostics::SimulationPerf;
use crate::gameplay::map::BattlefieldMap;
use crate::gameplay::objectives::{MissionObjectiveSet, MissionOutcome};
use crate::gameplay::orders::CombatOrderSource;
use crate::gameplay::simulation::SimulationClock;
use crate::gameplay::spatial::{BattlefieldPosition, Heading};
use crate::missions::{MissionDefinition, SelectedMission};
use crate::player::control::PlayerControl;
use crate::player::knowledge::{PlayerControlledUnit, PlayerTacticalKnowledge};
use crate::player::selection::{INFO_PANEL_WIDTH_PX, SelectedUnit};
use crate::ui::widgets::{
    TextButtonConfig, ToggleConfig, spawn_checkbox_toggle, spawn_text_button,
};
use bevy::camera::visibility::Visibility;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button, ValueChange, observe};
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
pub struct SelectedUnitTraceToggleText;

#[derive(Component)]
pub struct SelectedUnitTraceBody;

#[derive(Component)]
pub struct SelectedUnitTraceText;

#[derive(Component)]
pub struct SelectedUnitBeliefsToggleText;

#[derive(Component)]
pub struct SelectedUnitBeliefsBody;

#[derive(Component)]
pub struct SelectedUnitBeliefsText;

#[derive(Component, Clone, Copy)]
struct UnitDebugSectionToggle {
    section: UnitDebugSection,
}

#[derive(Debug, Clone, Copy)]
enum UnitDebugSection {
    Trace,
    Beliefs,
}

#[derive(Resource, Debug)]
pub struct UnitDebugPanelState {
    trace_open: bool,
    beliefs_open: bool,
}

impl Default for UnitDebugPanelState {
    fn default() -> Self {
        Self {
            trace_open: false,
            beliefs_open: false,
        }
    }
}

#[derive(Component)]
pub struct SimulationClockText;

#[derive(Component)]
pub struct SimulationPerfText;

#[derive(Component)]
pub struct SimulationPerfBreakdownPanel;

#[derive(Component)]
pub struct SimulationPerfBreakdownText;

#[derive(Component, Clone, Copy)]
struct SpawnSoldierAction {
    rank: Rank,
    role: Role,
    side: Side,
}

#[derive(Component, Clone, Copy)]
struct MissionMenuToggle {
    id: MenuId,
}

#[derive(Component)]
pub struct MissionOutcomeBanner;

#[derive(Component)]
pub struct MissionOutcomeText;

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
            .init_resource::<UnitDebugPanelState>()
            .add_systems(
                OnEnter(crate::GameState::MissionScreen),
                (setup_mission_ui, setup_selected_mission),
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
                    update_simulation_perf_breakdown,
                    update_mission_outcome_banner,
                )
                    .run_if(in_state(crate::GameState::MissionScreen)),
            );
    }
}

// ============================================================================
// SYSTEMS
// ============================================================================

fn handle_spawn_soldier_activate(
    activate: On<Activate>,
    mut commands: Commands,
    actions: Query<&SpawnSoldierAction>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };

    spawn_soldier(&mut commands, action.rank, action.role, action.side);
}

fn handle_unit_debug_section_toggle(
    activate: On<Activate>,
    mut debug_state: ResMut<UnitDebugPanelState>,
    toggles: Query<&UnitDebugSectionToggle>,
) {
    let Ok(toggle) = toggles.get(activate.entity) else {
        return;
    };

    match toggle.section {
        UnitDebugSection::Trace => debug_state.trace_open = !debug_state.trace_open,
        UnitDebugSection::Beliefs => debug_state.beliefs_open = !debug_state.beliefs_open,
    }
}

fn handle_mission_menu_toggle_change(
    value_change: On<ValueChange<bool>>,
    mut menu_state: ResMut<MenuState>,
    toggles: Query<&MissionMenuToggle>,
) {
    if let Ok(toggle) = toggles.get(value_change.source) {
        menu_state.set(toggle.id, value_change.value);
    }
}

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
    clock: Res<SimulationClock>,
    control: Res<PlayerControl>,
    knowledge: Res<PlayerTacticalKnowledge>,
    debug_state: Res<UnitDebugPanelState>,
    mut node_queries: ParamSet<(
        Query<&mut Node, With<SelectedUnitInfoPanel>>,
        Query<&mut Node, With<SelectedUnitTraceBody>>,
        Query<&mut Node, With<SelectedUnitBeliefsBody>>,
    )>,
    mut text_queries: ParamSet<(
        Query<&mut Text, With<SelectedUnitInfoText>>,
        Query<&mut Text, With<SelectedUnitTraceToggleText>>,
        Query<&mut Text, With<SelectedUnitTraceText>>,
        Query<&mut Text, With<SelectedUnitBeliefsToggleText>>,
        Query<&mut Text, With<SelectedUnitBeliefsText>>,
    )>,
    units: Query<(
        &Soldier,
        &Allegiance,
        &Health,
        &Mobility,
        &Inventory,
        &BattlefieldPosition,
        Option<&Heading>,
        Option<&VisualSensor>,
        Option<&PerceptionMemory>,
        Option<&DecisionTrace>,
        Option<&PlannerBelief>,
    )>,
) {
    let mut panel_query = node_queries.p0();
    let Ok(mut panel_node) = panel_query.single_mut() else {
        return;
    };

    let Some(entity) = selected.entity else {
        panel_node.display = Display::None;
        return;
    };

    let Ok((
        soldier,
        allegiance,
        health,
        mobility,
        inventory,
        _position,
        heading,
        visual_sensor,
        memory,
        trace,
        belief,
    )) = units.get(entity)
    else {
        panel_node.display = Display::None;
        return;
    };

    let Some(known) = knowledge.get(entity) else {
        panel_node.display = Display::None;
        return;
    };

    panel_node.display = Display::Flex;

    let is_current = known.last_reported_tick == clock.tick;
    let is_controlled_side = allegiance.side == control.side;
    let position_m = known.last_known_position_m;
    let heading_text = if is_current && is_controlled_side {
        heading
            .map(|Heading(angle)| format!("{angle:.2} rad"))
            .unwrap_or_else(|| "n/a".to_string())
    } else {
        "unknown".to_string()
    };
    let sensor_text = if is_current && is_controlled_side {
        visual_sensor
            .map(|sensor| {
                format!(
                    "Visual range: {:.0}m\nVisual FOV: {:.0}°",
                    sensor.range_m,
                    sensor.fov_radians.to_degrees()
                )
            })
            .unwrap_or_else(|| "Visual sensor: none".to_string())
    } else {
        "Visual sensor: unknown".to_string()
    };
    let contact_count = if is_current && is_controlled_side {
        memory
            .map(PerceptionMemory::unique_contact_count)
            .unwrap_or(0)
    } else {
        0
    };

    drop(panel_node);
    drop(panel_query);

    {
        let mut text_query = text_queries.p0();
        let Ok(mut text) = text_query.single_mut() else {
            return;
        };

        **text = format!(
            "Side: {:?}\nRank: {:?}\nRole: {:?}\n\nHealth: {}/{}\nSpeed: {}\nAmmo: {}\n\nPosition: ({:.1}m, {:.1}m)\nHeading: {}\n\n{}\nContacts: {}",
            allegiance.side,
            soldier.rank,
            soldier.role,
            health.current,
            health.max,
            mobility.speed,
            inventory.ammo_count(),
            position_m.x,
            position_m.y,
            heading_text,
            sensor_text,
            contact_count,
        );
    }

    {
        let mut toggle_query = text_queries.p1();
        if let Ok(mut toggle_text) = toggle_query.single_mut() {
            **toggle_text = format!(
                "{} Decision Trace",
                if debug_state.trace_open { "▾" } else { "▸" }
            );
        }
    }
    {
        let mut body_query = node_queries.p1();
        if let Ok(mut body_node) = body_query.single_mut() {
            body_node.display = if debug_state.trace_open {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
    {
        let mut trace_text_query = text_queries.p2();
        if let Ok(mut trace_text) = trace_text_query.single_mut() {
            **trace_text = format_trace_view(trace);
        }
    }

    {
        let mut toggle_query = text_queries.p3();
        if let Ok(mut toggle_text) = toggle_query.single_mut() {
            **toggle_text = format!(
                "{} Beliefs",
                if debug_state.beliefs_open {
                    "▾"
                } else {
                    "▸"
                }
            );
        }
    }
    {
        let mut body_query = node_queries.p2();
        if let Ok(mut body_node) = body_query.single_mut() {
            body_node.display = if debug_state.beliefs_open {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
    {
        let mut beliefs_text_query = text_queries.p4();
        if let Ok(mut beliefs_text) = beliefs_text_query.single_mut() {
            **beliefs_text = format_beliefs_view(belief);
        }
    }
}

fn format_trace_view(trace: Option<&DecisionTrace>) -> String {
    let Some(trace) = trace else {
        return "No decision trace component.".to_string();
    };

    let lines = trace
        .records()
        .rev()
        .take(8)
        .map(format_trace_record)
        .collect::<Vec<_>>();

    if lines.is_empty() {
        "No trace events yet.".to_string()
    } else {
        lines.join("\n\n")
    }
}

fn format_trace_record(record: &TraceRecord) -> String {
    format!(
        "{}  {}",
        format_sim_time(record.elapsed_s),
        format_trace_event(&record.event)
    )
}

fn format_trace_event(event: &TraceEvent) -> String {
    match event {
        TraceEvent::PlanCreated { root, mtr, steps } => {
            let mut lines = vec![format!("PlanCreated: {root} MTR {:?}", mtr.0)];
            lines.extend(steps.iter().map(|step| format!("  {step}")));
            lines.join("\n")
        }
        TraceEvent::PlanRejected { reason } => {
            format!("PlanRejected: {}", format_plan_rejection_reason(*reason))
        }
        TraceEvent::StepStarted {
            task,
            why,
            operator,
        } => format!("StepStarted: {task}\n  why: {why}\n  op: {operator}"),
        TraceEvent::StepFailed {
            task,
            failed_condition,
        } => format!("StepFailed: {task}\n  failed: {failed_condition}"),
        TraceEvent::Replanned { trigger } => {
            format!("Replanned: {}", format_replan_trigger(*trigger))
        }
        TraceEvent::PlanCompleted => "PlanCompleted".to_string(),
    }
}

fn format_replan_trigger(trigger: ReplanTrigger) -> &'static str {
    match trigger {
        ReplanTrigger::NoPlan => "NoPlan",
        ReplanTrigger::PlanCompleted => "PlanCompleted",
        ReplanTrigger::StepFailed => "StepFailed",
        ReplanTrigger::RelevantStateChanged => "RelevantStateChanged",
    }
}

fn format_plan_rejection_reason(reason: PlanRejectionReason) -> &'static str {
    match reason {
        PlanRejectionReason::NoValidPlan => "NoValidPlan",
        PlanRejectionReason::MtrNotBetter => "MtrNotBetter",
        PlanRejectionReason::ExternalOrderActive => "ExternalOrderActive",
    }
}

fn format_beliefs_view(belief: Option<&PlannerBelief>) -> String {
    let Some(belief) = belief else {
        return "No planner belief component.".to_string();
    };

    let state = &belief.state;
    let hostile = state.nearest_hostile.map_or_else(
        || "none".to_string(),
        |hostile| {
            format!(
                "{:?}\n  pos: ({:.1}, {:.1})m\n  confidence: {:.2}\n  kind: {:?}\n  staleness: {} ticks",
                hostile.entity,
                hostile.position_m.x,
                hostile.position_m.y,
                hostile.confidence,
                hostile.kind,
                state.tick.saturating_sub(hostile.last_seen_tick),
            )
        },
    );

    format!(
        "Health: {:.2}\nAmmo: {}\nUnder fire: {}\nMove target: {}\nNearest hostile: {}",
        state.health_frac, state.has_ammo, state.under_fire, state.has_move_target, hostile,
    )
}

fn format_sim_time(elapsed_s: f32) -> String {
    let total_seconds = elapsed_s.max(0.0);
    let minutes = (total_seconds / 60.0).floor() as u32;
    let seconds = total_seconds % 60.0;
    format!("T+{minutes:02}:{seconds:04.1}")
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

pub fn update_mission_outcome_banner(
    outcome: Res<MissionOutcome>,
    mut banner_query: Query<&mut Node, With<MissionOutcomeBanner>>,
    mut text_query: Query<(&mut Text, &mut TextColor), With<MissionOutcomeText>>,
) {
    if !outcome.is_changed() {
        return;
    }

    let Ok(mut banner_node) = banner_query.single_mut() else {
        return;
    };
    let Ok((mut text, mut text_color)) = text_query.single_mut() else {
        return;
    };

    match *outcome {
        MissionOutcome::InProgress => {
            banner_node.display = Display::None;
        }
        MissionOutcome::Victory => {
            banner_node.display = Display::Flex;
            **text = "VICTORY".to_string();
            *text_color = TextColor(Color::srgb(0.45, 1.0, 0.45));
        }
        MissionOutcome::Defeat => {
            banner_node.display = Display::Flex;
            **text = "DEFEAT".to_string();
            *text_color = TextColor(Color::srgb(1.0, 0.25, 0.2));
        }
    }
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

/// Toggle the per-phase perf breakdown when the diagnostics box is clicked.
fn handle_perf_panel_click(
    _click: On<Pointer<Click>>,
    mut panel_query: Query<&mut Node, With<SimulationPerfBreakdownPanel>>,
) {
    let Ok(mut node) = panel_query.single_mut() else {
        return;
    };

    node.display = match node.display {
        Display::None => Display::Flex,
        _ => Display::None,
    };
}

pub fn update_simulation_perf_breakdown(
    perf: Res<SimulationPerf>,
    panel_query: Query<&Node, With<SimulationPerfBreakdownPanel>>,
    mut text_query: Query<&mut Text, With<SimulationPerfBreakdownText>>,
) {
    if !perf.is_changed() {
        return;
    }

    // Skip formatting while hidden.
    let Ok(panel_node) = panel_query.single() else {
        return;
    };
    if panel_node.display == Display::None {
        return;
    }

    let Ok(mut text) = text_query.single_mut() else {
        return;
    };

    let phases = perf.phases_by_cost();
    let max_s = phases
        .first()
        .map(|(_, s)| *s)
        .unwrap_or(0.0)
        .max(f32::EPSILON);
    let total_s: f32 = phases.iter().map(|(_, s)| s).sum::<f32>().max(f32::EPSILON);

    let mut lines = String::new();
    for (name, ema_s) in &phases {
        let filled = ((ema_s / max_s) * 10.0).round().clamp(0.0, 10.0) as usize;
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled));
        lines.push_str(&format!(
            "{:<10} {:>6.2}ms [{}] {:>3.0}%\n",
            name,
            ema_s * 1000.0,
            bar,
            (ema_s / total_s) * 100.0,
        ));
    }

    **text = lines;
}

/// Setup the entire mission UI hierarchy using flexbox
pub fn setup_mission_ui(mut commands: Commands) {
    commands
        .spawn((
            MissionScreenRoot,
            MissionOutcomeBanner,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                left: Val::Percent(0.0),
                right: Val::Percent(0.0),
                top: Val::Percent(38.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(20.0)),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                MissionOutcomeText,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(72.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });

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
            // Lift above the full-screen root container so this box both renders
            // and receives pointer picks on top of it.
            GlobalZIndex(10),
            observe(handle_perf_panel_click),
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

            // Per-phase breakdown, hidden until the diagnostics box is clicked.
            parent
                .spawn((
                    SimulationPerfBreakdownPanel,
                    Node {
                        display: Display::None,
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::top(Val::Px(4.0)),
                        ..default()
                    },
                ))
                .with_children(|panel| {
                    panel.spawn((
                        SimulationPerfBreakdownText,
                        Text::new(""),
                        TextFont {
                            font_size: FontSize::Px(13.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.85, 0.85, 0.85)),
                    ));
                });
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
                    spawn_checkbox_toggle(
                        sidebar,
                        ToggleConfig {
                            label: "U".to_string(),
                            checked: false,
                            width: Val::Px(180.0),
                            height: Val::Px(50.0),
                            ..default()
                        },
                        (
                            MissionMenuToggle { id: MenuId::Unit },
                            observe(handle_mission_menu_toggle_change),
                        ),
                    );

                    spawn_checkbox_toggle(
                        sidebar,
                        ToggleConfig {
                            label: "S".to_string(),
                            checked: false,
                            width: Val::Px(180.0),
                            height: Val::Px(50.0),
                            ..default()
                        },
                        (
                            MissionMenuToggle {
                                id: MenuId::Settings,
                            },
                            observe(handle_mission_menu_toggle_change),
                        ),
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
                            spawn_text_button(
                                unit_bar,
                                TextButtonConfig {
                                    label: "Spawn Private".to_string(),
                                    ..default()
                                },
                                (
                                    SpawnSoldierAction {
                                        rank: Rank::Private,
                                        role: Role::Rifleman,
                                        side: Side::Blue,
                                    },
                                    observe(handle_spawn_soldier_activate),
                                ),
                            );

                            spawn_text_button(
                                unit_bar,
                                TextButtonConfig {
                                    label: "Spawn Sergeant".to_string(),
                                    ..default()
                                },
                                (
                                    SpawnSoldierAction {
                                        rank: Rank::Sergeant,
                                        role: Role::Rifleman,
                                        side: Side::Blue,
                                    },
                                    observe(handle_spawn_soldier_activate),
                                ),
                            );

                            spawn_text_button(
                                unit_bar,
                                TextButtonConfig {
                                    label: "Spawn Medic".to_string(),
                                    ..default()
                                },
                                (
                                    SpawnSoldierAction {
                                        rank: Rank::Private,
                                        role: Role::Medic,
                                        side: Side::Blue,
                                    },
                                    observe(handle_spawn_soldier_activate),
                                ),
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

                    panel
                        .spawn((
                            Button,
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(30.0),
                                justify_content: JustifyContent::FlexStart,
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.12, 0.12, 0.12)),
                            UnitDebugSectionToggle {
                                section: UnitDebugSection::Trace,
                            },
                            observe(handle_unit_debug_section_toggle),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                SelectedUnitTraceToggleText,
                                Text::new("▸ Decision Trace"),
                                TextFont {
                                    font_size: FontSize::Px(15.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });

                    panel
                        .spawn((
                            SelectedUnitTraceBody,
                            Node {
                                display: Display::None,
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                padding: UiRect::left(Val::Px(6.0)),
                                ..default()
                            },
                        ))
                        .with_children(|body| {
                            body.spawn((
                                SelectedUnitTraceText,
                                Text::new(""),
                                TextFont {
                                    font_size: FontSize::Px(12.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.78, 0.78, 0.78)),
                            ));
                        });

                    panel
                        .spawn((
                            Button,
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(30.0),
                                justify_content: JustifyContent::FlexStart,
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.12, 0.12, 0.12)),
                            UnitDebugSectionToggle {
                                section: UnitDebugSection::Beliefs,
                            },
                            observe(handle_unit_debug_section_toggle),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                SelectedUnitBeliefsToggleText,
                                Text::new("▸ Beliefs"),
                                TextFont {
                                    font_size: FontSize::Px(15.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });

                    panel
                        .spawn((
                            SelectedUnitBeliefsBody,
                            Node {
                                display: Display::None,
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                padding: UiRect::left(Val::Px(6.0)),
                                ..default()
                            },
                        ))
                        .with_children(|body| {
                            body.spawn((
                                SelectedUnitBeliefsText,
                                Text::new(""),
                                TextFont {
                                    font_size: FontSize::Px(12.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.78, 0.78, 0.78)),
                            ));
                        });
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

pub fn setup_selected_mission(mut commands: Commands, selected: Option<Res<SelectedMission>>) {
    let Some(selected) = selected else {
        panic!("MISSION DOESNT EXIST: MissionScreen entered without SelectedMission");
    };

    spawn_mission(&mut commands, selected.mission);
}

pub fn spawn_mission(commands: &mut Commands, mission: &MissionDefinition) {
    info!(
        "Spawning mission: {} ({}) on map: {}",
        mission.name, mission.id, mission.map.name
    );
    commands.insert_resource(BattlefieldMap::from_definition(mission.map));
    commands.insert_resource(SimulationClock::default());
    commands.insert_resource(PlayerTacticalKnowledge::default());
    commands.insert_resource(SelectedUnit::default());
    commands.insert_resource(MissionObjectiveSet::from_slices(
        mission.victory_conditions,
        mission.defeat_conditions,
    ));
    commands.insert_resource(MissionOutcome::InProgress);

    let mut entities_by_unit_id = HashMap::new();
    let mut side_by_entity = HashMap::new();

    for unit in mission.units {
        let entity = spawn_soldier_at(
            commands,
            unit.rank,
            unit.role,
            unit.side,
            Vec2::new(unit.position_meters[0], unit.position_meters[1]),
            unit.heading_radians,
        );

        commands.entity(entity).insert(UnitIdentity { id: unit.id });
        entities_by_unit_id.insert(unit.id, entity);
        side_by_entity.insert(entity, unit.side);

        if unit.side == Side::Blue && matches!(unit.rank, Rank::Sergeant) {
            commands.entity(entity).insert(PlayerControlledUnit);
        }
    }

    commands.insert_resource(CommandForest::from_assignments(
        mission.command_assignments,
        &entities_by_unit_id,
        |entity| side_by_entity.get(&entity).copied(),
    ));
}

/// Spawn a soldier entity (gameplay logic)
pub fn spawn_soldier(commands: &mut Commands, rank: Rank, role: Role, side: Side) {
    let position = default_spawn_position_m(side);
    let heading = match side {
        Side::Blue => std::f32::consts::PI,
        Side::Red => 0.0,
    };

    spawn_soldier_at(commands, rank, role, side, position, heading);
}

fn default_spawn_position_m(side: Side) -> Vec2 {
    match side {
        // Keep UI-spawned units off the center hill so terrain occlusion is visually legible,
        // and close enough to the demo command group to be in voice contact.
        Side::Blue => Vec2::new(80.0, -45.0),
        Side::Red => Vec2::new(-80.0, -45.0),
    }
}

pub fn spawn_soldier_at(
    commands: &mut Commands,
    rank: Rank,
    role: Role,
    side: Side,
    position: Vec2,
    heading_radians: f32,
) -> Entity {
    let entity = commands
        .spawn((
            MissionEntity,
            Soldier { rank, role },
            Alive,
            Allegiance { side },
            Health {
                current: 100,
                max: 100,
            },
            Mobility { speed: 1 },
            Inventory {
                items: vec![Item {
                    kind: ItemKind::Ammo,
                    count: 120,
                }],
            },
            BattlefieldPosition(position),
            Heading(heading_radians),
            VisualSensor::default(),
            AuditorySensor::default(),
            EyeHeight::default(),
            SensorSignature::default(),
        ))
        .insert((
            Weapon::default_rifle(),
            CombatState::default(),
            Marksmanship::default(),
            CombatOrder::HoldFire,
            CombatOrderSource::doctrine(),
            PerceptionMemory::default(),
            VoiceComms::default(),
            CommsLinks::default(),
            Autonomous,
            DecisionTrace::default(),
            PlannerBelief::default(),
            DomainRef(DomainId::Soldier),
        ))
        .id();

    info!(
        "Soldier spawned! Rank: {:?}, Role: {:?}, Side: {:?}",
        rank, role, side
    );

    entity
}
