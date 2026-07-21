use super::unit_ai_debug;
use crate::actors::units::{Allegiance, Health, Inventory, Mobility, Soldier};
use crate::ai::perception::{PerceptionMemory, VisualSensor};
use crate::gameplay::simulation::SimulationClock;
use crate::gameplay::spatial::{BattlefieldPosition, Heading};
use crate::player::control::PlayerControl;
use crate::player::knowledge::PlayerTacticalKnowledge;
use crate::player::selection::{INFO_PANEL_WIDTH_PX, SelectedUnit};
use bevy::prelude::*;

#[derive(Component)]
struct SelectedUnitInfoPanel;

#[derive(Component)]
struct SelectedUnitInfoText;

pub(super) fn register(app: &mut App) {
    app.add_systems(
        Update,
        update_selected_unit_info_panel.run_if(in_state(crate::GameState::MissionScreen)),
    );
}

fn update_selected_unit_info_panel(
    selected: Res<SelectedUnit>,
    clock: Res<SimulationClock>,
    control: Res<PlayerControl>,
    knowledge: Res<PlayerTacticalKnowledge>,
    mut panel_query: Query<&mut Node, With<SelectedUnitInfoPanel>>,
    mut text_query: Query<&mut Text, With<SelectedUnitInfoText>>,
    units: Query<(
        &Soldier,
        &Allegiance,
        &Health,
        &Mobility,
        &Inventory,
        &BattlefieldPosition,
        Option<&Heading>,
        Option<&VisualSensor>,
        Option<&PerceptionMemory>,
    )>,
    changed_units: Query<
        (),
        Or<(
            Changed<Soldier>,
            Changed<Allegiance>,
            Changed<Health>,
            Changed<Mobility>,
            Changed<Inventory>,
            Changed<BattlefieldPosition>,
            Changed<Heading>,
            Changed<VisualSensor>,
            Changed<PerceptionMemory>,
        )>,
    >,
) {
    let unit_changed = selected
        .entity
        .is_some_and(|entity| changed_units.get(entity).is_ok());
    if !selected.is_changed()
        && !clock.is_changed()
        && !control.is_changed()
        && !knowledge.is_changed()
        && !unit_changed
    {
        return;
    }

    let Ok(mut panel_node) = panel_query.single_mut() else {
        return;
    };
    let Some(entity) = selected.entity else {
        set_display_if_changed(&mut panel_node, Display::None);
        return;
    };
    let Ok((
        soldier,
        allegiance,
        health,
        mobility,
        inventory,
        _position,
        heading,
        visual_sensor,
        memory,
    )) = units.get(entity)
    else {
        set_display_if_changed(&mut panel_node, Display::None);
        return;
    };
    let Some(known) = knowledge.get(entity) else {
        set_display_if_changed(&mut panel_node, Display::None);
        return;
    };

    set_display_if_changed(&mut panel_node, Display::Flex);
    let is_current = known.last_reported_tick == clock.tick;
    let is_controlled_side = allegiance.side == control.side;
    let position_m = known.last_known_position_m;
    let heading_text = if is_current && is_controlled_side {
        heading
            .map(|Heading(angle)| format!("{angle:.2} rad"))
            .unwrap_or_else(|| "n/a".to_string())
    } else {
        "unknown".to_string()
    };
    let sensor_text = if is_current && is_controlled_side {
        visual_sensor
            .map(|sensor| {
                format!(
                    "Visual range: {:.0}m\nVisual FOV: {:.0}°",
                    sensor.range_m,
                    sensor.fov_radians.to_degrees()
                )
            })
            .unwrap_or_else(|| "Visual sensor: none".to_string())
    } else {
        "Visual sensor: unknown".to_string()
    };
    let contact_count = if is_current && is_controlled_side {
        memory
            .map(PerceptionMemory::unique_contact_count)
            .unwrap_or(0)
    } else {
        0
    };

    let next = format!(
        "Side: {:?}\nRank: {:?}\nRole: {:?}\n\nHealth: {}/{}\nSpeed: {}\nAmmo: {}\n\nPosition: ({:.1}m, {:.1}m)\nHeading: {}\n\n{}\nContacts: {}",
        allegiance.side,
        soldier.rank,
        soldier.role,
        health.current,
        health.max,
        mobility.speed,
        inventory.ammo_count(),
        position_m.x,
        position_m.y,
        heading_text,
        sensor_text,
        contact_count,
    );
    if let Ok(mut text) = text_query.single_mut()
        && text.0 != next
    {
        text.0 = next;
    }
}

fn set_display_if_changed(node: &mut Node, display: Display) {
    if node.display != display {
        node.display = display;
    }
}

pub(super) fn spawn(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            SelectedUnitInfoPanel,
            Node {
                display: Display::None,
                width: Val::Px(INFO_PANEL_WIDTH_PX),
                height: Val::Auto,
                margin: UiRect::top(Val::Px(76.0)),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.08)),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("Selected Unit"),
                TextFont {
                    font_size: FontSize::Px(22.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            panel.spawn((
                SelectedUnitInfoText,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.85, 0.85)),
            ));
            unit_ai_debug::spawn(panel);
        });
}
