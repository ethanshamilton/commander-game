#![doc = include_str!("../../docs/player/selection.md")]

use crate::GameState;
use crate::gameplay::command::CommandForest;
use crate::gameplay::comms::CommsGraph;
use crate::gameplay::measurements::{meters, to_meters};
use crate::gameplay::simulation::{SimulationClock, UnitOrder};
use crate::player::control::PlayerControl;
use crate::player::knowledge::{PlayerControlledUnit, PlayerTacticalKnowledge};
use crate::units::{Allegiance, Soldier};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

pub const INFO_PANEL_WIDTH_PX: f32 = 240.0;
const SIDEBAR_WIDTH_PX: f32 = 200.0;
const BOTTOM_BAR_HEIGHT_PX: f32 = 100.0;
const SELECTION_RADIUS_M: f32 = 2.0;

pub struct SelectionPlugin;

impl Plugin for SelectionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedUnit>().add_systems(
            Update,
            (select_unit, issue_move_order).run_if(in_state(GameState::MissionScreen)),
        );
    }
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct SelectedUnit {
    pub entity: Option<Entity>,
}

fn select_unit(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    control: Res<PlayerControl>,
    clock: Res<SimulationClock>,
    knowledge: Res<PlayerTacticalKnowledge>,
    mut selected: ResMut<SelectedUnit>,
) {
    if !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };

    let Some(cursor_position) = window.cursor_position() else {
        return;
    };

    if cursor_position.x <= SIDEBAR_WIDTH_PX || cursor_position.y <= BOTTOM_BAR_HEIGHT_PX {
        return;
    }

    if selected.entity.is_some() && cursor_position.x >= window.width() - INFO_PANEL_WIDTH_PX {
        return;
    }

    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };

    let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position) else {
        return;
    };

    let selection_radius = meters(SELECTION_RADIUS_M);
    selected.entity = knowledge
        .units
        .iter()
        .filter(|unit| unit.side != control.side || unit.last_reported_tick == clock.tick)
        .filter_map(|unit| {
            let distance = unit
                .last_known_position_m
                .map(meters)
                .distance(world_position);
            (distance <= selection_radius).then_some((unit.entity, distance))
        })
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(entity, _)| entity);
}

fn issue_move_order(
    mut commands: Commands,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    selected: Res<SelectedUnit>,
    control: Res<PlayerControl>,
    clock: Res<SimulationClock>,
    knowledge: Res<PlayerTacticalKnowledge>,
    graph: Res<CommsGraph>,
    command_forest: Res<CommandForest>,
    controlled: Query<Entity, With<PlayerControlledUnit>>,
    units: Query<&Allegiance, With<Soldier>>,
) {
    if !mouse_buttons.just_pressed(MouseButton::Right) {
        return;
    }

    let Some(entity) = selected.entity else {
        return;
    };

    let Ok(allegiance) = units.get(entity) else {
        return;
    };

    if allegiance.side != control.side || !knowledge.is_current(entity, clock.tick) {
        return;
    };

    let Ok(controlled_entity) = controlled.single() else {
        return;
    };

    if !graph.can_reach(controlled_entity, entity, control.side, |entity| {
        units.get(entity).ok().map(|allegiance| allegiance.side)
    }) {
        return;
    }

    if !command_forest.can_command(controlled_entity, entity) {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };

    let Some(cursor_position) = window.cursor_position() else {
        return;
    };

    if cursor_position.x <= SIDEBAR_WIDTH_PX || cursor_position.y <= BOTTOM_BAR_HEIGHT_PX {
        return;
    }

    if cursor_position.x >= window.width() - INFO_PANEL_WIDTH_PX {
        return;
    }

    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };

    let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position) else {
        return;
    };

    commands.entity(entity).insert(UnitOrder::MoveTo {
        destination_m: world_position.map(to_meters),
    });
}
