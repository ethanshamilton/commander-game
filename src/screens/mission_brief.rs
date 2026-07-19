#![doc = include_str!("../../docs/screens/mission_brief.md")]

use crate::GameState;
use crate::missions::SelectedMission;
use crate::ui::widgets::{TextButtonConfig, spawn_text_button};
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, observe};

pub struct MissionBriefScreenPlugin;

impl Plugin for MissionBriefScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MissionBrief), setup_mission_brief)
            .add_systems(OnExit(GameState::MissionBrief), cleanup_mission_brief);
    }
}

#[derive(Component)]
struct MissionBriefRoot;

#[derive(Component)]
struct StartMissionAction;

#[derive(Component)]
struct BackToMissionSelectAction;

fn setup_mission_brief(mut commands: Commands, selected: Option<Res<SelectedMission>>) {
    commands
        .spawn((
            MissionBriefRoot,
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
                    Text::new("MISSION DOES NOT EXIST"),
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
                Text::new(selected.mission.name),
                TextFont {
                    font_size: FontSize::Px(44.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            parent.spawn((
                Text::new(selected.mission.briefing),
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
                            label: "Start Mission".to_string(),
                            width: Val::Px(220.0),
                            height: Val::Px(56.0),
                            ..default()
                        },
                        (StartMissionAction, observe(start_mission)),
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
        (BackToMissionSelectAction, observe(back_to_mission_select)),
    );
}

fn start_mission(
    activate: On<Activate>,
    selected: Option<Res<SelectedMission>>,
    mut next_state: ResMut<NextState<GameState>>,
    actions: Query<&StartMissionAction>,
) {
    if actions.get(activate.entity).is_err() {
        return;
    }

    if selected.is_none() {
        panic!("MISSION DOES NOT EXIST: Start Mission activated without SelectedMission");
    }

    next_state.set(GameState::MissionScreen);
}

fn back_to_mission_select(
    activate: On<Activate>,
    mut next_state: ResMut<NextState<GameState>>,
    actions: Query<&BackToMissionSelectAction>,
) {
    if actions.get(activate.entity).is_ok() {
        next_state.set(GameState::MissionSelect);
    }
}

fn cleanup_mission_brief(mut commands: Commands, roots: Query<Entity, With<MissionBriefRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}
