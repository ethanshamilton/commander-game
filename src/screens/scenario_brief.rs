#![doc = include_str!("../../docs/screens/scenario_brief.md")]

use crate::GameState;
use crate::scenarios::SelectedScenario;
use crate::ui::widgets::{TextButtonConfig, spawn_text_button};
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, observe};

pub struct ScenarioBriefScreenPlugin;

impl Plugin for ScenarioBriefScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::ScenarioBrief), setup_scenario_brief)
            .add_systems(OnExit(GameState::ScenarioBrief), cleanup_scenario_brief);
    }
}

#[derive(Component)]
struct ScenarioBriefRoot;

#[derive(Component)]
struct StartScenarioAction;

#[derive(Component)]
struct BackToScenarioSelectAction;

fn setup_scenario_brief(mut commands: Commands, selected: Option<Res<SelectedScenario>>) {
    commands
        .spawn((
            ScenarioBriefRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(Val::Px(48.0)),
                row_gap: Val::Px(24.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            let Some(selected) = selected else {
                parent.spawn((
                    Text::new("SCENARIO DOES NOT EXIST"),
                    TextFont {
                        font_size: FontSize::Px(48.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.15, 0.1)),
                ));

                spawn_back_button(parent);
                return;
            };

            parent.spawn((
                Text::new(selected.scenario.name),
                TextFont {
                    font_size: FontSize::Px(44.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            parent.spawn((
                Text::new(selected.scenario.briefing),
                TextFont {
                    font_size: FontSize::Px(22.0),
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.85, 0.85)),
                Node {
                    width: Val::Px(720.0),
                    ..default()
                },
            ));

            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(16.0),
                    ..default()
                })
                .with_children(|buttons| {
                    spawn_back_button(buttons);
                    spawn_text_button(
                        buttons,
                        TextButtonConfig {
                            label: "Start Scenario".to_string(),
                            width: Val::Px(220.0),
                            height: Val::Px(56.0),
                            ..default()
                        },
                        (StartScenarioAction, observe(start_scenario)),
                    );
                });
        });
}

fn spawn_back_button(parent: &mut ChildSpawnerCommands) {
    spawn_text_button(
        parent,
        TextButtonConfig {
            label: "Back".to_string(),
            width: Val::Px(160.0),
            height: Val::Px(56.0),
            ..default()
        },
        (BackToScenarioSelectAction, observe(back_to_scenario_select)),
    );
}

fn start_scenario(
    activate: On<Activate>,
    selected: Option<Res<SelectedScenario>>,
    mut next_state: ResMut<NextState<GameState>>,
    actions: Query<&StartScenarioAction>,
) {
    if actions.get(activate.entity).is_err() {
        return;
    }

    if selected.is_none() {
        panic!("SCENARIO DOES NOT EXIST: Start Scenario activated without SelectedScenario");
    }

    next_state.set(GameState::ScenarioScreen);
}

fn back_to_scenario_select(
    activate: On<Activate>,
    mut next_state: ResMut<NextState<GameState>>,
    actions: Query<&BackToScenarioSelectAction>,
) {
    if actions.get(activate.entity).is_ok() {
        next_state.set(GameState::ScenarioSelect);
    }
}

fn cleanup_scenario_brief(mut commands: Commands, roots: Query<Entity, With<ScenarioBriefRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}
