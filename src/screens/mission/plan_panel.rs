use super::menu::{Menu, MenuId};
use crate::gameplay::command_plans::CommandPlan;
use crate::player::plan_placement::{PlanPlacementState, SelectedPlan, expiry_duration_ticks};
use crate::ui::widgets::{TextButtonConfig, spawn_text_button};
use bevy::prelude::*;
use bevy::text::{EditableText, TextCursorStyle};
use bevy::ui_widgets::{Activate, observe};

#[derive(Component)]
struct BeginHoldLinePlacementAction;

#[derive(Component)]
struct PlanList;

#[derive(Component, Clone, Copy)]
struct SelectPlanAction {
    plan: Entity,
}

#[derive(Component)]
struct RenameSelectedPlanAction;

#[derive(Component)]
struct PlanRenameInput;

#[derive(Component)]
struct PlanExpiryInput;

#[derive(Component)]
struct CommandPlanAssignmentStatus;

#[derive(Resource, Debug, Clone)]
struct CommandPlanAssignmentFeedback(String);

impl Default for CommandPlanAssignmentFeedback {
    fn default() -> Self {
        Self("Select a plan and squad leader to assign.".into())
    }
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<CommandPlanAssignmentFeedback>()
        .add_systems(
            Update,
            (update_plan_list, update_plan_assignment_status)
                .run_if(in_state(crate::GameState::MissionScreen)),
        );
}

fn begin_hold_line_placement(
    activate: On<Activate>,
    actions: Query<(), With<BeginHoldLinePlacementAction>>,
    inputs: Query<&EditableText, With<PlanExpiryInput>>,
    mut placement: ResMut<PlanPlacementState>,
    mut feedback: ResMut<CommandPlanAssignmentFeedback>,
) {
    if actions.get(activate.entity).is_err() {
        return;
    }
    let Ok(input) = inputs.single() else {
        return;
    };
    let expiry_text = input.value().to_string();
    let Ok(minutes) = expiry_text.trim().parse::<u64>() else {
        feedback.0 = "Expiry must be a non-negative whole number of minutes.".into();
        return;
    };
    let Ok(duration_ticks) = expiry_duration_ticks(minutes) else {
        feedback.0 = "Expiry duration is too large.".into();
        return;
    };

    placement.begin_hold_line(duration_ticks);
}

fn select_plan(
    activate: On<Activate>,
    actions: Query<&SelectPlanAction>,
    plans: Query<(), With<CommandPlan>>,
    mut selected: ResMut<SelectedPlan>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    if plans.get(action.plan).is_ok() {
        selected.entity = Some(action.plan);
        selected.preview = true;
        selected.assignment_mode = true;
    }
}

fn rename_selected_plan(
    activate: On<Activate>,
    actions: Query<(), With<RenameSelectedPlanAction>>,
    selected: Res<SelectedPlan>,
    inputs: Query<&EditableText, With<PlanRenameInput>>,
    mut plans: Query<(&mut CommandPlan, &mut Name)>,
) {
    if actions.get(activate.entity).is_err() {
        return;
    }
    let Some(entity) = selected.entity else {
        return;
    };
    let Ok(input) = inputs.single() else {
        return;
    };
    let label = input.value().to_string();
    if label.trim().is_empty() {
        return;
    }
    let Ok((mut plan, mut name)) = plans.get_mut(entity) else {
        return;
    };

    plan.label = label;
    *name = Name::new(plan.label.clone());
}

fn update_plan_assignment_status(
    feedback: Res<CommandPlanAssignmentFeedback>,
    selected: Res<SelectedPlan>,
    mut statuses: Query<&mut Text, With<CommandPlanAssignmentStatus>>,
) {
    if !feedback.is_changed() && !selected.is_changed() {
        return;
    }
    let text = if selected.assignment_mode {
        "Assign Plan Mode: select a squad leader."
    } else {
        &feedback.0
    };
    for mut status in &mut statuses {
        **status = text.to_string();
    }
}

fn update_plan_list(
    mut commands: Commands,
    plans: Query<(Entity, Ref<CommandPlan>)>,
    selected: Res<SelectedPlan>,
    lists: Query<Entity, With<PlanList>>,
) {
    if !selected.is_changed() && !plans.iter().any(|(_, plan)| plan.is_changed()) {
        return;
    }

    let Ok(list) = lists.single() else {
        return;
    };

    let mut entries: Vec<_> = plans.iter().collect();
    entries.sort_by_key(|(_, plan)| plan.id);
    commands.entity(list).despawn_children();
    commands.entity(list).with_children(|parent| {
        if entries.is_empty() {
            parent.spawn((
                Text::new("No plans"),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ));
        }

        for (entity, plan) in entries {
            let marker = if selected.entity == Some(entity) {
                "* "
            } else {
                ""
            };
            spawn_text_button(
                parent,
                TextButtonConfig {
                    label: format!("{marker}{}", plan.label),
                    width: Val::Px(180.0),
                    height: Val::Px(34.0),
                    text_size: 14.0,
                    ..default()
                },
                (SelectPlanAction { plan: entity }, observe(select_plan)),
            );
        }
    });
}

pub(super) fn spawn(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Menu { id: MenuId::Plan },
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.12, 0.08)),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("Expiry minutes (0 = none)"),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.8, 0.65)),
            ));
            panel.spawn((
                EditableText::new("5"),
                TextCursorStyle::default(),
                PlanExpiryInput,
                Node {
                    width: Val::Px(180.0),
                    height: Val::Px(30.0),
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.08, 0.08, 0.08)),
                BorderColor::all(Color::srgb(0.55, 0.55, 0.45)),
            ));
            spawn_text_button(
                panel,
                TextButtonConfig {
                    label: "Hold Line".to_string(),
                    width: Val::Px(180.0),
                    height: Val::Px(38.0),
                    text_size: 16.0,
                    ..default()
                },
                (
                    BeginHoldLinePlacementAction,
                    observe(begin_hold_line_placement),
                ),
            );
            panel.spawn((
                CommandPlanAssignmentStatus,
                Text::new("Select a plan and squad leader to assign."),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.8, 0.65)),
            ));
            panel.spawn((
                EditableText::new(""),
                TextCursorStyle::default(),
                PlanRenameInput,
                Node {
                    width: Val::Px(180.0),
                    height: Val::Px(30.0),
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.08, 0.08, 0.08)),
                BorderColor::all(Color::srgb(0.55, 0.55, 0.45)),
            ));
            spawn_text_button(
                panel,
                TextButtonConfig {
                    label: "Apply name".to_string(),
                    width: Val::Px(180.0),
                    height: Val::Px(30.0),
                    text_size: 13.0,
                    ..default()
                },
                (RenameSelectedPlanAction, observe(rename_selected_plan)),
            );
            panel.spawn((
                Text::new("Plans"),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.85, 0.75)),
            ));
            panel.spawn((
                PlanList,
                Node {
                    width: Val::Px(180.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                },
            ));
        });
}
