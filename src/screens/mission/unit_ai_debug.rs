use crate::ai::htn::synthesis::PlannerBelief;
use crate::ai::htn::trace::{
    DecisionTrace, PlanRejectionReason, ReplanTrigger, TraceEvent, TraceRecord,
};
use crate::player::selection::SelectedUnit;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button, observe};

#[derive(Component)]
struct TraceToggleText;
#[derive(Component)]
struct TraceBody;
#[derive(Component)]
struct TraceText;
#[derive(Component)]
struct BeliefsToggleText;
#[derive(Component)]
struct BeliefsBody;
#[derive(Component)]
struct BeliefsText;

#[derive(Component, Clone, Copy)]
struct SectionToggle {
    section: DebugSection,
}

#[derive(Debug, Clone, Copy)]
enum DebugSection {
    Trace,
    Beliefs,
}

#[derive(Resource, Debug, Default)]
struct DebugPanelState {
    trace_open: bool,
    beliefs_open: bool,
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<DebugPanelState>().add_systems(
        Update,
        (update_chrome, update_trace, update_beliefs)
            .run_if(in_state(crate::GameState::MissionScreen)),
    );
}

fn handle_section_toggle(
    activate: On<Activate>,
    mut state: ResMut<DebugPanelState>,
    toggles: Query<&SectionToggle>,
) {
    let Ok(toggle) = toggles.get(activate.entity) else {
        return;
    };

    match toggle.section {
        DebugSection::Trace => state.trace_open = !state.trace_open,
        DebugSection::Beliefs => state.beliefs_open = !state.beliefs_open,
    }
}

fn update_chrome(
    state: Res<DebugPanelState>,
    mut node_queries: ParamSet<(
        Query<&mut Node, With<TraceBody>>,
        Query<&mut Node, With<BeliefsBody>>,
    )>,
    mut text_queries: ParamSet<(
        Query<&mut Text, With<TraceToggleText>>,
        Query<&mut Text, With<BeliefsToggleText>>,
    )>,
) {
    if !state.is_changed() {
        return;
    }

    if let Ok(mut body) = node_queries.p0().single_mut() {
        set_display_if_changed(
            &mut body,
            if state.trace_open {
                Display::Flex
            } else {
                Display::None
            },
        );
    }
    if let Ok(mut body) = node_queries.p1().single_mut() {
        set_display_if_changed(
            &mut body,
            if state.beliefs_open {
                Display::Flex
            } else {
                Display::None
            },
        );
    }
    if let Ok(mut text) = text_queries.p0().single_mut() {
        set_text_if_changed(
            &mut text,
            if state.trace_open {
                "▾ Decision Trace"
            } else {
                "▸ Decision Trace"
            },
        );
    }
    if let Ok(mut text) = text_queries.p1().single_mut() {
        set_text_if_changed(
            &mut text,
            if state.beliefs_open {
                "▾ Beliefs"
            } else {
                "▸ Beliefs"
            },
        );
    }
}

fn update_trace(
    selected: Res<SelectedUnit>,
    state: Res<DebugPanelState>,
    traces: Query<&DecisionTrace>,
    changed_traces: Query<(), Changed<DecisionTrace>>,
    mut text_query: Query<&mut Text, With<TraceText>>,
) {
    if !state.trace_open {
        return;
    }
    let Some(entity) = selected.entity else {
        return;
    };
    if !selected.is_changed() && !state.is_changed() && changed_traces.get(entity).is_err() {
        return;
    }
    if let Ok(mut text) = text_query.single_mut() {
        let next = format_trace_view(traces.get(entity).ok());
        if text.0 != next {
            text.0 = next;
        }
    }
}

fn update_beliefs(
    selected: Res<SelectedUnit>,
    state: Res<DebugPanelState>,
    beliefs: Query<&PlannerBelief>,
    changed_beliefs: Query<(), Changed<PlannerBelief>>,
    mut text_query: Query<&mut Text, With<BeliefsText>>,
) {
    if !state.beliefs_open {
        return;
    }
    let Some(entity) = selected.entity else {
        return;
    };
    if !selected.is_changed() && !state.is_changed() && changed_beliefs.get(entity).is_err() {
        return;
    }
    if let Ok(mut text) = text_query.single_mut() {
        let next = format_beliefs_view(beliefs.get(entity).ok());
        if text.0 != next {
            text.0 = next;
        }
    }
}

fn set_display_if_changed(node: &mut Node, display: Display) {
    if node.display != display {
        node.display = display;
    }
}

fn set_text_if_changed(text: &mut Text, next: &str) {
    if text.0 != next {
        text.0 = next.to_string();
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
        |hostile| format!(
            "{:?}\n  pos: ({:.1}, {:.1})m\n  confidence: {:.2}\n  kind: {:?}\n  staleness: {} ticks",
            hostile.entity, hostile.position_m.x, hostile.position_m.y, hostile.confidence,
            hostile.kind, state.tick.saturating_sub(hostile.last_seen_tick),
        ),
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

pub(super) fn spawn(parent: &mut ChildSpawnerCommands) {
    spawn_section(parent, DebugSection::Trace, "▸ Decision Trace");
    spawn_section(parent, DebugSection::Beliefs, "▸ Beliefs");
}

fn spawn_section(parent: &mut ChildSpawnerCommands, section: DebugSection, label: &'static str) {
    parent
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
            SectionToggle { section },
            observe(handle_section_toggle),
        ))
        .with_children(|button| {
            if matches!(section, DebugSection::Trace) {
                button.spawn((
                    TraceToggleText,
                    Text::new(label),
                    TextFont {
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            } else {
                button.spawn((
                    BeliefsToggleText,
                    Text::new(label),
                    TextFont {
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            }
        });

    let body = Node {
        display: Display::None,
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        padding: UiRect::left(Val::Px(6.0)),
        ..default()
    };
    if matches!(section, DebugSection::Trace) {
        parent.spawn((TraceBody, body)).with_children(|body| {
            body.spawn((
                TraceText,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.78, 0.78, 0.78)),
            ));
        });
    } else {
        parent.spawn((BeliefsBody, body)).with_children(|body| {
            body.spawn((
                BeliefsText,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.78, 0.78, 0.78)),
            ));
        });
    }
}
