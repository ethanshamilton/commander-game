use super::{MissionScreenRoot, hud, menu, performance_overlay, plan_panel, unit_panel};
use bevy::prelude::*;

pub(super) fn setup_mission_ui(mut commands: Commands) {
    hud::spawn(&mut commands);
    performance_overlay::spawn(&mut commands);

    commands
        .spawn((
            MissionScreenRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
        ))
        .with_children(|root| {
            menu::spawn_sidebar(root);

            root.spawn(Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|main_area| {
                main_area.spawn(Node {
                    flex_grow: 1.0,
                    ..default()
                });
                plan_panel::spawn(main_area);
            });

            unit_panel::spawn(root);
        });
}
