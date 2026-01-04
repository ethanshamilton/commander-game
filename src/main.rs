mod actions;
mod units;

use actions::*;
use bevy::prelude::*;
use units::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, (setup, setup_ui))
        .add_systems(Update, interaction_system)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.insert_resource(ClearColor(Color::srgb(0.1, 0.12, 0.14)));
}

fn setup_ui(mut commands: Commands) {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|parent| {
            spawn_button(
                parent,
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
                parent,
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
                parent,
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
}
