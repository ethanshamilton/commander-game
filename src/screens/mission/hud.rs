use super::MissionScreenRoot;
use crate::gameplay::objectives::MissionOutcome;
use crate::ui::active_action::{ActiveActionPanel, ActiveActionText};
use bevy::prelude::*;

#[derive(Component)]
struct MissionOutcomeBanner;
#[derive(Component)]
struct MissionOutcomeText;

pub(super) fn register(app: &mut App) {
    app.add_systems(
        Update,
        update_mission_outcome_banner.run_if(in_state(crate::GameState::MissionScreen)),
    );
}

fn update_mission_outcome_banner(
    outcome: Res<MissionOutcome>,
    mut banners: Query<&mut Node, With<MissionOutcomeBanner>>,
    mut texts: Query<(&mut Text, &mut TextColor), With<MissionOutcomeText>>,
) {
    if !outcome.is_changed() {
        return;
    }
    let Ok(mut banner) = banners.single_mut() else {
        return;
    };
    let Ok((mut text, mut color)) = texts.single_mut() else {
        return;
    };
    match *outcome {
        MissionOutcome::InProgress => banner.display = Display::None,
        MissionOutcome::Victory => {
            banner.display = Display::Flex;
            **text = "VICTORY".to_string();
            *color = TextColor(Color::srgb(0.45, 1.0, 0.45));
        }
        MissionOutcome::Defeat => {
            banner.display = Display::Flex;
            **text = "DEFEAT".to_string();
            *color = TextColor(Color::srgb(1.0, 0.25, 0.2));
        }
    }
}

pub(super) fn spawn(commands: &mut Commands) {
    commands
        .spawn((
            MissionScreenRoot,
            MissionOutcomeBanner,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                left: Val::Percent(0.0),
                right: Val::Percent(0.0),
                top: Val::Percent(38.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(20.0)),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                MissionOutcomeText,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(72.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });

    commands
        .spawn((
            MissionScreenRoot,
            ActiveActionPanel,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                width: Val::Px(300.0),
                height: Val::Px(48.0),
                right: Val::Px(16.0),
                bottom: Val::Px(116.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.02, 0.95)),
            BorderColor::all(Color::srgb(1.0, 0.9, 0.15)),
            GlobalZIndex(100),
        ))
        .with_children(|parent| {
            parent.spawn((
                ActiveActionText,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.9, 0.15)),
            ));
        });
}
