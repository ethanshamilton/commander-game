#![doc = include_str!("../../docs/screens/main_menu.md")]

use crate::GameState;
use crate::ui::widgets::{TextButtonConfig, spawn_text_button};
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, observe};

pub struct MainMenuScreenPlugin;

impl Plugin for MainMenuScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), setup_main_menu)
            .add_systems(OnExit(GameState::MainMenu), cleanup_main_menu);
    }
}

#[derive(Component)]
struct MainMenuRoot;

#[derive(Component)]
struct GoToMissionSelectAction;

#[derive(Component)]
struct GoToSettingsAction;

fn setup_main_menu(mut commands: Commands) {
    commands
        .spawn((
            MainMenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(18.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Commander"),
                TextFont {
                    font_size: FontSize::Px(64.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            spawn_text_button(
                parent,
                TextButtonConfig {
                    label: "Tutorial Missions".to_string(),
                    width: Val::Px(260.0),
                    height: Val::Px(58.0),
                    ..default()
                },
                (GoToMissionSelectAction, observe(go_to_mission_select)),
            );

            spawn_text_button(
                parent,
                TextButtonConfig {
                    label: "Settings".to_string(),
                    width: Val::Px(260.0),
                    height: Val::Px(58.0),
                    ..default()
                },
                (GoToSettingsAction, observe(go_to_settings)),
            );
        });
}

fn go_to_mission_select(
    activate: On<Activate>,
    mut next_state: ResMut<NextState<GameState>>,
    actions: Query<&GoToMissionSelectAction>,
) {
    if actions.get(activate.entity).is_ok() {
        next_state.set(GameState::MissionSelect);
    }
}

fn go_to_settings(
    activate: On<Activate>,
    mut next_state: ResMut<NextState<GameState>>,
    actions: Query<&GoToSettingsAction>,
) {
    if actions.get(activate.entity).is_ok() {
        next_state.set(GameState::Settings);
    }
}

fn cleanup_main_menu(mut commands: Commands, roots: Query<Entity, With<MainMenuRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}
