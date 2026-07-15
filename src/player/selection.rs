#![doc = include_str!("../../docs/player/selection.md")]

use crate::GameState;
use crate::actors::units::Alive;
use crate::gameplay::combat::CombatOrder;
use crate::gameplay::command::CommandForest;
use crate::gameplay::measurements::{meters, to_meters};
use crate::gameplay::orders::{CombatOrderSource, UnitOrderSource};
use crate::gameplay::packets::{
    Address, OrderCommand, Outbox, PacketIdAllocator, PacketPayload, SeenPackets,
};
use crate::gameplay::simulation::{SimulationClock, UnitOrder};
use crate::player::control::PlayerControl;
use crate::player::knowledge::{PlayerControlledUnit, PlayerTacticalKnowledge};
use crate::player::mission_placement::{MissionPlacementState, PlayerInputSet, SelectedMission};
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
            (select_unit, issue_contextual_order)
                .in_set(PlayerInputSet::Selection)
                .run_if(in_state(GameState::ScenarioScreen)),
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
    knowledge: Res<PlayerTacticalKnowledge>,
    placement: Res<MissionPlacementState>,
    mut selected: ResMut<SelectedUnit>,
    mut selected_mission: ResMut<SelectedMission>,
) {
    if !mouse_buttons.just_pressed(MouseButton::Left) || placement.is_active() {
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
    let clicked_unit = knowledge
        .units
        .iter()
        .filter_map(|unit| {
            let distance = unit
                .last_known_position_m
                .map(meters)
                .distance(world_position);
            (distance <= selection_radius).then_some((unit.entity, distance))
        })
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(entity, _)| entity);

    // Unit and mission selections are distinct inspection modes. An actual
    // map-unit selection exits mission-preview mode; clicks that do not select
    // a unit (including UI clicks that leak through picking) must not erase it.
    if clicked_unit.is_some() {
        selected_mission.entity = None;
    }
    selected.entity = clicked_unit;
}

fn issue_contextual_order(
    mut commands: Commands,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    selected: Res<SelectedUnit>,
    placement: Res<MissionPlacementState>,
    control: Res<PlayerControl>,
    clock: Res<SimulationClock>,
    knowledge: Res<PlayerTacticalKnowledge>,
    command_forest: Res<CommandForest>,
    mut packet_ids: ResMut<PacketIdAllocator>,
    controlled: Query<Entity, (With<PlayerControlledUnit>, With<Alive>)>,
    mut packet_outboxes: Query<
        (&mut Outbox, &mut SeenPackets),
        (With<PlayerControlledUnit>, With<Alive>),
    >,
) {
    if !mouse_buttons.just_pressed(MouseButton::Right) || placement.is_active() {
        return;
    }

    let Some(entity) = selected.entity else {
        return;
    };

    let Some(selected_unit) = knowledge.get(entity) else {
        return;
    };

    if selected_unit.side != control.side {
        return;
    }

    let Ok(controlled_entity) = controlled.single() else {
        return;
    };

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

    let order =
        if let Some(target) = hostile_unit_at_cursor(&knowledge, control.side, world_position) {
            OrderCommand::Combat(CombatOrder::FireAt { target })
        } else {
            OrderCommand::Unit(UnitOrder::MoveTo {
                destination_m: world_position.map(to_meters),
            })
        };

    if entity == controlled_entity {
        match order {
            OrderCommand::Unit(order) => {
                commands
                    .entity(entity)
                    .insert((order, UnitOrderSource::player()));
            }
            OrderCommand::Combat(order) => {
                commands
                    .entity(entity)
                    .insert((order, CombatOrderSource::player()));
            }
        }
        return;
    }

    let Ok((mut outbox, mut seen)) = packet_outboxes.get_mut(controlled_entity) else {
        return;
    };

    outbox.send(
        &mut seen,
        &mut packet_ids,
        controlled_entity,
        Address::Direct(entity),
        clock.tick,
        PacketPayload::OrderCommand(order),
    );
}

fn hostile_unit_at_cursor(
    knowledge: &PlayerTacticalKnowledge,
    player_side: crate::actors::units::Side,
    world_position: Vec2,
) -> Option<Entity> {
    let selection_radius = meters(SELECTION_RADIUS_M);

    knowledge
        .units
        .iter()
        .filter(|unit| unit.side != player_side)
        .filter_map(|unit| {
            let distance = unit
                .last_known_position_m
                .map(meters)
                .distance(world_position);
            (distance <= selection_radius).then_some((unit.entity, distance))
        })
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(entity, _)| entity)
}
