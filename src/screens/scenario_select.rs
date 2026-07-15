#![doc = include_str!("../../docs/screens/scenario_select.md")]

use crate::GameState;
use crate::scenarios::{ScenarioDefinition, SelectedScenario, TUTORIAL_SCENARIOS};
use crate::ui::widgets::{ListRowConfig, TextButtonConfig, spawn_list_row, spawn_text_button};
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, observe};

pub struct ScenarioSelectScreenPlugin;

impl Plugin for ScenarioSelectScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::ScenarioSelect), setup_scenario_select)
            .add_systems(OnExit(GameState::ScenarioSelect), cleanup_scenario_select);
    }
}

#[derive(Component)]
struct ScenarioSelectRoot;

#[derive(Component, Clone, Copy)]
struct SelectScenarioAction {
    scenario: &'static ScenarioDefinition,
}

#[derive(Component)]
struct GoToMainMenuAction;

fn setup_scenario_select(mut commands: Commands) {
    commands
        .spawn((
            ScenarioSelectRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(40.0)),
                row_gap: Val::Px(24.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Tutorial Scenarios"),
                TextFont {
                    font_size: FontSize::Px(46.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            parent
                .spawn(Node {
                    width: Val::Px(560.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|list| {
                    for scenario in TUTORIAL_SCENARIOS {
                        spawn_list_row(
                            list,
                            ListRowConfig {
                                label: scenario.name.to_string(),
                                ..default()
                            },
                            (SelectScenarioAction { scenario }, observe(select_scenario)),
                        );
                    }
                });

            spawn_text_button(
                parent,
                TextButtonConfig {
                    label: "Back".to_string(),
                    width: Val::Px(160.0),
                    height: Val::Px(48.0),
                    text_size: 18.0,
                    ..default()
                },
                (GoToMainMenuAction, observe(go_to_main_menu)),
            );
        });
}

fn select_scenario(
    activate: On<Activate>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    actions: Query<&SelectScenarioAction>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };

    commands.insert_resource(SelectedScenario {
        scenario: action.scenario,
    });
    next_state.set(GameState::ScenarioBrief);
}

fn go_to_main_menu(
    activate: On<Activate>,
    mut next_state: ResMut<NextState<GameState>>,
    actions: Query<&GoToMainMenuAction>,
) {
    if actions.get(activate.entity).is_ok() {
        next_state.set(GameState::MainMenu);
    }
}

fn cleanup_scenario_select(mut commands: Commands, roots: Query<Entity, With<ScenarioSelectRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}
