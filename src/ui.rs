use crate::actions::*;
use crate::units::*;
use bevy::prelude::*;

/// Bottom bar with unit spawning buttons
pub fn setup_unit_bar(mut commands: Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Px(100.0),
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::FlexEnd,
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
