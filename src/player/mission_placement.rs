#![doc = include_str!("../../docs/player/mission_placement.md")]

use crate::GameState;
use crate::gameplay::missions::{
    MissionArea, MissionAssignees, MissionIdAllocator, MissionKind, MissionPlan, TacticalMission,
};
use crate::gameplay::simulation::SimulationClock;
use crate::screens::scenario::ScenarioScoped;
use crate::ui::active_action::ActiveAction;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

const SIDEBAR_WIDTH_PX: f32 = 200.0;
const BOTTOM_BAR_HEIGHT_PX: f32 = 100.0;
const INFO_PANEL_WIDTH_PX: f32 = 240.0;

pub struct MissionPlacementPlugin;

impl Plugin for MissionPlacementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MissionPlacementState>()
            .init_resource::<SelectedMission>()
            .init_resource::<MissionLabelAllocator>()
            .configure_sets(
                Update,
                (PlayerInputSet::MissionPlacement, PlayerInputSet::Selection).chain(),
            )
            .add_systems(
                Update,
                (
                    cancel_mission_placement,
                    place_hold_line_points,
                    refresh_selected_mission,
                    sync_mission_active_action,
                )
                    .chain()
                    .in_set(PlayerInputSet::MissionPlacement)
                    .run_if(in_state(GameState::ScenarioScreen)),
            )
            .add_systems(OnExit(GameState::ScenarioScreen), reset_mission_ui_state);
    }
}

/// The in-progress map-placement interaction. `None` means ordinary selection
/// and contextual orders may consume map clicks.
#[derive(Resource, Debug, Default, Clone)]
pub struct MissionPlacementState {
    pub active: Option<MissionPlacement>,
}

impl MissionPlacementState {
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn begin_hold_line(&mut self) {
        self.active = Some(MissionPlacement::hold_line());
    }

    pub fn cancel(&mut self) {
        self.active = None;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MissionPlacement {
    pub kind: MissionKind,
    pub phase: HoldLinePlacementPhase,
    pub line_start_m: Option<Vec2>,
    pub line_end_m: Option<Vec2>,
}

impl MissionPlacement {
    pub fn hold_line() -> Self {
        Self {
            kind: MissionKind::HoldLine,
            phase: HoldLinePlacementPhase::LineStart,
            line_start_m: None,
            line_end_m: None,
        }
    }

    pub fn instruction(&self) -> &'static str {
        match self.phase {
            HoldLinePlacementPhase::LineStart => "Create Line Start",
            HoldLinePlacementPhase::LineEnd => "Create Line End",
            HoldLinePlacementPhase::RallyPoint => "Create Rally Point",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldLinePlacementPhase {
    LineStart,
    LineEnd,
    RallyPoint,
}

/// Tactical mission selected for future assignment and highlighted in overlays.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct SelectedMission {
    /// The tactical mission currently chosen for actions such as assignment.
    pub entity: Option<Entity>,
    /// True only while the mission list explicitly previews this mission.
    pub preview: bool,
    /// Selecting a mission enables assignment to the next valid map unit click.
    pub assignment_mode: bool,
}

/// Orders player-visible labels independently of scenario-local mission IDs.
#[derive(Resource, Debug, Default)]
pub struct MissionLabelAllocator {
    next: u64,
}

impl MissionLabelAllocator {
    pub fn next_label(&mut self, kind: MissionKind) -> String {
        self.next += 1;
        format!("{} {}", kind.display_name(), self.next)
    }

    fn reset(&mut self) {
        self.next = 0;
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerInputSet {
    MissionPlacement,
    Selection,
}

fn cancel_mission_placement(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut placement: ResMut<MissionPlacementState>,
    mut selected: ResMut<SelectedMission>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        placement.cancel();
        selected.assignment_mode = false;
        selected.preview = false;
    }
}

fn place_hold_line_points(
    mut commands: Commands,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    clock: Res<SimulationClock>,
    mut ids: ResMut<MissionIdAllocator>,
    mut labels: ResMut<MissionLabelAllocator>,
    mut placement: ResMut<MissionPlacementState>,
    mut selected: ResMut<SelectedMission>,
) {
    if !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let Some(point_m) = map_click_in_meters(&windows, &cameras) else {
        return;
    };
    let Some(active) = placement.active.as_mut() else {
        return;
    };

    debug_assert_eq!(active.kind, MissionKind::HoldLine);
    match active.phase {
        HoldLinePlacementPhase::LineStart => {
            active.line_start_m = Some(point_m);
            active.phase = HoldLinePlacementPhase::LineEnd;
        }
        HoldLinePlacementPhase::LineEnd => {
            active.line_end_m = Some(point_m);
            active.phase = HoldLinePlacementPhase::RallyPoint;
        }
        HoldLinePlacementPhase::RallyPoint => {
            let Some(from_m) = active.line_start_m else {
                placement.cancel();
                return;
            };
            let Some(to_m) = active.line_end_m else {
                placement.cancel();
                return;
            };

            let id = ids.allocate();
            let plan = hold_line_plan(
                id,
                labels.next_label(MissionKind::HoldLine),
                from_m,
                to_m,
                point_m,
                clock.tick,
            );

            if let Err(error) = plan.validate() {
                warn!(?error, "discarded invalid tactical mission placement");
                placement.cancel();
                return;
            }

            let entity = commands
                .spawn((
                    TacticalMission,
                    ScenarioScoped,
                    Name::new(plan.label.clone()),
                    MissionAssignees::default(),
                    plan,
                ))
                .id();
            selected.entity = Some(entity);
            selected.preview = true;
            selected.assignment_mode = true;
            placement.cancel();
        }
    }
}

fn hold_line_plan(
    id: crate::gameplay::missions::MissionId,
    label: String,
    from_m: Vec2,
    to_m: Vec2,
    rally_point_m: Vec2,
    created_tick: u64,
) -> MissionPlan {
    MissionPlan {
        id,
        label,
        kind: MissionKind::HoldLine,
        area: MissionArea::Line { from_m, to_m },
        rally_point_m,
        expires_at: None,
        created_tick,
    }
}

fn refresh_selected_mission(
    mut selected: ResMut<SelectedMission>,
    missions: Query<(), (With<TacticalMission>, With<MissionPlan>)>,
) {
    if selected
        .entity
        .is_some_and(|entity| missions.get(entity).is_err())
    {
        selected.entity = None;
        selected.preview = false;
        selected.assignment_mode = false;
    }
}

fn sync_mission_active_action(
    placement: Res<MissionPlacementState>,
    selected: Res<SelectedMission>,
    mut action: ResMut<ActiveAction>,
) {
    if let Some(placement) = placement.active.as_ref() {
        action.set(placement.instruction());
    } else if selected.assignment_mode {
        action.set("Assign Mission: Select Squad Leader");
    } else {
        action.clear();
    }
}

fn reset_mission_ui_state(
    mut placement: ResMut<MissionPlacementState>,
    mut selected: ResMut<SelectedMission>,
    mut labels: ResMut<MissionLabelAllocator>,
    mut action: ResMut<ActiveAction>,
) {
    placement.cancel();
    selected.entity = None;
    selected.preview = false;
    selected.assignment_mode = false;
    labels.reset();
    action.clear();
}

/// Converts a left-click in the playable map region into meters. UI areas and
/// the selected-unit panel are deliberately excluded.
pub fn map_click_in_meters(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<(&Camera, &GlobalTransform), With<Camera2d>>,
) -> Option<Vec2> {
    let window = windows.single().ok()?;
    let cursor = window.cursor_position()?;
    if cursor.x <= SIDEBAR_WIDTH_PX
        || cursor.y <= BOTTOM_BAR_HEIGHT_PX
        || cursor.x >= window.width() - INFO_PANEL_WIDTH_PX
    {
        return None;
    }

    let (camera, transform) = cameras.single().ok()?;
    camera
        .viewport_to_world_2d(transform, cursor)
        .ok()
        .map(|position| position / crate::gameplay::measurements::BEVY_UNITS_PER_METER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_line_placement_progresses_through_the_expected_instructions() {
        let mut placement = MissionPlacement::hold_line();
        assert_eq!(placement.instruction(), "Create Line Start");

        placement.line_start_m = Some(Vec2::ZERO);
        placement.phase = HoldLinePlacementPhase::LineEnd;
        assert_eq!(placement.instruction(), "Create Line End");

        placement.line_end_m = Some(Vec2::X);
        placement.phase = HoldLinePlacementPhase::RallyPoint;
        assert_eq!(placement.instruction(), "Create Rally Point");
    }

    #[test]
    fn completed_hold_line_placement_builds_a_valid_plan() {
        let plan = hold_line_plan(
            crate::gameplay::missions::MissionId(0),
            "Hold Line 1".into(),
            Vec2::new(-5.0, 0.0),
            Vec2::new(5.0, 0.0),
            Vec2::new(0.0, -3.0),
            42,
        );

        assert_eq!(plan.validate(), Ok(()));
        assert_eq!(plan.label, "Hold Line 1");
    }

    #[test]
    fn escape_cancels_assignment_mode_without_discarding_the_selected_mission() {
        let mut placement = MissionPlacementState::default();
        let mut selected = SelectedMission {
            entity: Some(Entity::PLACEHOLDER),
            preview: true,
            assignment_mode: true,
        };

        placement.cancel();
        selected.assignment_mode = false;
        selected.preview = false;

        assert_eq!(selected.entity, Some(Entity::PLACEHOLDER));
        assert!(!selected.assignment_mode);
        assert!(!selected.preview);
    }

    #[test]
    fn mission_ui_state_is_reset_on_scenario_exit() {
        let mut placement = MissionPlacementState {
            active: Some(MissionPlacement::hold_line()),
        };
        let mut selected = SelectedMission {
            entity: Some(Entity::PLACEHOLDER),
            preview: true,
            assignment_mode: true,
        };

        placement.cancel();
        selected.entity = None;
        selected.preview = false;
        selected.assignment_mode = false;

        assert!(!placement.is_active());
        assert_eq!(selected.entity, None);
        assert!(!selected.preview);
    }
}
