use super::MissionScreenRoot;
use crate::gameplay::command::UnitIdentity;
use crate::gameplay::command_succession::{CommandSucceeded, SuccessionNotice};
use crate::gameplay::objectives::MissionOutcome;
use crate::gameplay::simulation::SimulationClock;
use crate::player::knowledge::{CONTACT_RECENCY_TTL_TICKS, PlayerTacticalKnowledge};
use crate::ui::active_action::{ActiveActionPanel, ActiveActionText};
use bevy::prelude::*;

#[derive(Component)]
struct MissionOutcomeBanner;
#[derive(Component)]
struct MissionOutcomeText;
#[derive(Component)]
struct SuccessionNoticeText;

pub(super) fn register(app: &mut App) {
    app.add_systems(
        Update,
        (update_mission_outcome_banner, update_succession_notice)
            .run_if(in_state(crate::GameState::MissionScreen)),
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

fn update_succession_notice(
    mut successions: MessageReader<CommandSucceeded>,
    clock: Res<SimulationClock>,
    knowledge: Res<PlayerTacticalKnowledge>,
    identities: Query<&UnitIdentity>,
    mut notice: ResMut<SuccessionNotice>,
    mut texts: Query<&mut Text, With<SuccessionNoticeText>>,
) {
    for succession in successions.read() {
        let Some(successor) = succession.successor else {
            continue;
        };
        if !knowledge.is_recently_reported(
            succession.deceased,
            clock.tick,
            CONTACT_RECENCY_TTL_TICKS,
        ) || !knowledge.is_recently_reported(successor, clock.tick, CONTACT_RECENCY_TTL_TICKS)
        {
            continue;
        }
        let predecessor = identities
            .get(succession.deceased)
            .map(|identity| identity.id.0)
            .unwrap_or("Leader");
        let successor_name = identities
            .get(successor)
            .map(|identity| identity.id.0)
            .unwrap_or("Successor");
        notice.text = Some(format!(
            "{predecessor} killed; {successor_name} assumed command"
        ));
        notice.tick = clock.tick;
    }

    if clock.tick.saturating_sub(notice.tick) > 100 {
        notice.text = None;
    }
    if let Ok(mut text) = texts.single_mut() {
        text.0 = notice.text.clone().unwrap_or_default();
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

    commands.spawn((
        MissionScreenRoot,
        SuccessionNoticeText,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(22.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.9, 0.35)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(18.0),
            left: Val::Percent(30.0),
            right: Val::Percent(30.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
    ));

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::command::UnitId;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn hidden_succession_does_not_create_player_notice() {
        let mut world = World::new();
        world.init_resource::<SimulationClock>();
        world.init_resource::<PlayerTacticalKnowledge>();
        world.init_resource::<SuccessionNotice>();
        world.init_resource::<Messages<CommandSucceeded>>();
        let deceased = world
            .spawn(UnitIdentity {
                id: UnitId("enemy_leader"),
            })
            .id();
        let successor = world
            .spawn(UnitIdentity {
                id: UnitId("enemy_successor"),
            })
            .id();
        world.spawn((SuccessionNoticeText, Text::new("")));
        world.write_message(CommandSucceeded {
            squad: deceased,
            deceased,
            successor: Some(successor),
            tick: 1,
            squad_revision: 1,
        });

        world.run_system_once(update_succession_notice).unwrap();

        assert!(world.resource::<SuccessionNotice>().text.is_none());
    }
}
