use super::MissionScreenRoot;
use crate::gameplay::diagnostics::SimulationPerf;
use crate::gameplay::simulation::SimulationClock;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy::ui_widgets::observe;

#[derive(Component)]
struct SimulationClockText;
#[derive(Component)]
struct RenderPerfText;
#[derive(Component)]
struct SimulationPerfText;
#[derive(Component)]
struct SimulationPerfBreakdownPanel;
#[derive(Component)]
struct SimulationPerfBreakdownText;

pub(super) fn register(app: &mut App) {
    app.add_systems(
        Update,
        (
            update_clock,
            update_render_perf,
            update_simulation_perf,
            update_breakdown,
        )
            .run_if(in_state(crate::GameState::MissionScreen)),
    );
}

fn update_clock(
    clock: Res<SimulationClock>,
    mut query: Query<&mut Text, With<SimulationClockText>>,
) {
    if !clock.is_changed() {
        return;
    }
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    let minutes = (clock.elapsed_s / 60.0).floor() as u32;
    let seconds = (clock.elapsed_s % 60.0).floor() as u32;
    let paused = if clock.paused { " PAUSED" } else { "" };
    **text = format!("T+{minutes:02}:{seconds:02}  tick {}{paused}", clock.tick);
}

fn update_render_perf(
    diagnostics: Res<DiagnosticsStore>,
    mut query: Query<(&mut Text, &mut TextColor), With<RenderPerfText>>,
) {
    const TARGET_FPS: f64 = 60.0;
    let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed().or_else(|| diagnostic.value()))
    else {
        return;
    };
    let Ok((mut text, mut color)) = query.single_mut() else {
        return;
    };
    let attainment = fps / TARGET_FPS;
    let filled = (attainment * 10.0).round().clamp(0.0, 10.0) as usize;
    let meter = format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled));
    **text = format!(
        "RENDER {fps:03.0} FPS / {TARGET_FPS:03.0} FPS [{meter}] {:03.0}%",
        attainment * 100.0
    );
    *color = TextColor(render_perf_color(attainment));
}

fn update_simulation_perf(
    perf: Res<SimulationPerf>,
    mut query: Query<(&mut Text, &mut TextColor), With<SimulationPerfText>>,
) {
    if !perf.is_changed() {
        return;
    }
    let Ok((mut text, mut color)) = query.single_mut() else {
        return;
    };
    let filled = (perf.utilization * 10.0).round().clamp(0.0, 10.0) as usize;
    let meter = format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled));
    **text = format!(
        "SIM {:04.1}ms / {:04.1}ms [{}] {:03.0}%",
        perf.last_tick_s * 1000.0,
        perf.tick_budget_s * 1000.0,
        meter,
        perf.utilization * 100.0,
    );
    *color = TextColor(simulation_perf_color(perf.utilization));
}

fn render_perf_color(attainment: f64) -> Color {
    if attainment >= 1.0 {
        Color::srgb(0.7, 1.0, 0.7)
    } else if attainment >= 0.8 {
        Color::srgb(1.0, 0.9, 0.0)
    } else if attainment >= 0.5 {
        Color::srgb(1.0, 0.55, 0.0)
    } else {
        Color::srgb(1.0, 0.1, 0.1)
    }
}

fn simulation_perf_color(utilization: f32) -> Color {
    if utilization >= 1.0 {
        Color::srgb(1.0, 0.1, 0.1)
    } else if utilization >= 0.8 {
        Color::srgb(1.0, 0.55, 0.0)
    } else if utilization >= 0.5 {
        Color::srgb(1.0, 0.9, 0.0)
    } else {
        Color::srgb(0.7, 1.0, 0.7)
    }
}

fn handle_click(
    _click: On<Pointer<Click>>,
    mut query: Query<&mut Node, With<SimulationPerfBreakdownPanel>>,
) {
    let Ok(mut node) = query.single_mut() else {
        return;
    };
    node.display = if node.display == Display::None {
        Display::Flex
    } else {
        Display::None
    };
}

fn update_breakdown(
    perf: Res<SimulationPerf>,
    panels: Query<&Node, With<SimulationPerfBreakdownPanel>>,
    mut texts: Query<&mut Text, With<SimulationPerfBreakdownText>>,
) {
    if !perf.is_changed() {
        return;
    }
    let Ok(panel) = panels.single() else {
        return;
    };
    if panel.display == Display::None {
        return;
    }
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    let phases = perf.phases_by_cost();
    let max_s = phases
        .first()
        .map(|(_, s)| *s)
        .unwrap_or(0.0)
        .max(f32::EPSILON);
    let total_s = phases.iter().map(|(_, s)| s).sum::<f32>().max(f32::EPSILON);
    let mut lines = String::new();
    for (name, ema_s) in &phases {
        let filled = ((ema_s / max_s) * 10.0).round().clamp(0.0, 10.0) as usize;
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled));
        lines.push_str(&format!(
            "{:<10} {:>6.2}ms [{}] {:>3.0}%\n",
            name,
            ema_s * 1000.0,
            bar,
            (ema_s / total_s) * 100.0
        ));
    }
    **text = lines;
}

pub(super) fn spawn(commands: &mut Commands) {
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
            GlobalZIndex(10),
            observe(handle_click),
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
                RenderPerfText,
                Text::new("RENDER 000 FPS / 060 FPS [░░░░░░░░░░] 000%"),
                TextFont {
                    font_size: FontSize::Px(14.0),
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
}
