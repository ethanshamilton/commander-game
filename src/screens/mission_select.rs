#![doc = include_str!("../../docs/screens/mission_select.md")]

use crate::GameState;
use crate::missions::{MissionDefinition, SelectedMission, TUTORIAL_MISSIONS};
use crate::ui::widgets::{ListRowConfig, TextButtonConfig, spawn_list_row, spawn_text_button};
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, observe};

pub struct MissionSelectScreenPlugin;

impl Plugin for MissionSelectScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MissionSelect), setup_mission_select)
            .add_systems(OnExit(GameState::MissionSelect), cleanup_mission_select);
    }
}

#[derive(Component)]
struct MissionSelectRoot;

#[derive(Component, Clone, Copy)]
struct SelectMissionAction {
    mission: &'static MissionDefinition,
}

#[derive(Component)]
struct GoToMainMenuAction;

fn setup_mission_select(mut commands: Commands) {
    commands
        .spawn((
            MissionSelectRoot,
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
                Text::new("Tutorial Missions"),
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
                    for mission in TUTORIAL_MISSIONS {
                        spawn_list_row(
                            list,
                            ListRowConfig {
                                label: mission.name.to_string(),
                                ..default()
                            },
                            (SelectMissionAction { mission }, observe(select_mission)),
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

fn select_mission(
    activate: On<Activate>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    actions: Query<&SelectMissionAction>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };

    commands.insert_resource(SelectedMission {
        mission: action.mission,
    });
    next_state.set(GameState::MissionBrief);
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

fn cleanup_mission_select(mut commands: Commands, roots: Query<Entity, With<MissionSelectRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}
