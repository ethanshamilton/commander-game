#![doc = include_str!("../../docs/player/plan_placement.md")]

use crate::GameState;
use crate::gameplay::command_plans::{
    CommandPlan, CommandPlanArea, CommandPlanAssignees, CommandPlanIdAllocator, CommandPlanKind,
};
use crate::gameplay::simulation::{SIMULATION_TICK_HZ, SimulationClock};
use crate::screens::mission::MissionScoped;
use crate::ui::active_action::ActiveAction;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

const SIDEBAR_WIDTH_PX: f32 = 200.0;
const BOTTOM_BAR_HEIGHT_PX: f32 = 100.0;
const INFO_PANEL_WIDTH_PX: f32 = 240.0;
pub const SIMULATION_TICKS_PER_MINUTE: u64 = SIMULATION_TICK_HZ as u64 * 60;

/// Convert a player-entered duration to ticks. Zero deliberately means that
/// the plan has no expiration.
pub fn expiry_duration_ticks(minutes: u64) -> Result<Option<u64>, &'static str> {
    if minutes == 0 {
        return Ok(None);
    }
    minutes
        .checked_mul(SIMULATION_TICKS_PER_MINUTE)
        .map(Some)
        .ok_or("expiry duration is too large")
}

pub struct PlanPlacementPlugin;

impl Plugin for PlanPlacementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlanPlacementState>()
            .init_resource::<SelectedPlan>()
            .init_resource::<PlanLabelAllocator>()
            .configure_sets(
                Update,
                (PlayerInputSet::PlanPlacement, PlayerInputSet::Selection).chain(),
            )
            .add_systems(
                Update,
                (
                    cancel_plan_placement,
                    place_hold_line_points,
                    refresh_selected_plan,
                    sync_plan_active_action,
                )
                    .chain()
                    .in_set(PlayerInputSet::PlanPlacement)
                    .run_if(in_state(GameState::MissionScreen)),
            )
            .add_systems(OnExit(GameState::MissionScreen), reset_plan_ui_state);
    }
}

/// The in-progress map-placement interaction. `None` means ordinary selection
/// and contextual orders may consume map clicks.
#[derive(Resource, Debug, Default, Clone)]
pub struct PlanPlacementState {
    pub active: Option<PlanPlacement>,
}

impl PlanPlacementState {
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn begin_hold_line(&mut self, expiry_duration_ticks: Option<u64>) {
        self.active = Some(PlanPlacement::hold_line(expiry_duration_ticks));
    }

    pub fn cancel(&mut self) {
        self.active = None;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanPlacement {
    pub kind: CommandPlanKind,
    pub phase: HoldLinePlacementPhase,
    pub line_start_m: Option<Vec2>,
    pub line_end_m: Option<Vec2>,
    /// `None` means the plan does not expire.
    pub expiry_duration_ticks: Option<u64>,
}

impl PlanPlacement {
    pub fn hold_line(expiry_duration_ticks: Option<u64>) -> Self {
        Self {
            kind: CommandPlanKind::HoldLine,
            phase: HoldLinePlacementPhase::LineStart,
            line_start_m: None,
            line_end_m: None,
            expiry_duration_ticks,
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

/// Tactical plan selected for future assignment and highlighted in overlays.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct SelectedPlan {
    /// The tactical plan currently chosen for actions such as assignment.
    pub entity: Option<Entity>,
    /// True only while the plan list explicitly previews this plan.
    pub preview: bool,
    /// Selecting a plan enables assignment to the next valid map unit click.
    pub assignment_mode: bool,
}

/// Orders player-visible labels independently of mission-local plan IDs.
#[derive(Resource, Debug, Default)]
pub struct PlanLabelAllocator {
    next: u64,
}

impl PlanLabelAllocator {
    pub fn next_label(&mut self, kind: CommandPlanKind) -> String {
        self.next += 1;
        format!("{} {}", kind.display_name(), self.next)
    }

    fn reset(&mut self) {
        self.next = 0;
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerInputSet {
    PlanPlacement,
    Selection,
}

fn cancel_plan_placement(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut placement: ResMut<PlanPlacementState>,
    mut selected: ResMut<SelectedPlan>,
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
    mut ids: ResMut<CommandPlanIdAllocator>,
    mut labels: ResMut<PlanLabelAllocator>,
    mut placement: ResMut<PlanPlacementState>,
    mut selected: ResMut<SelectedPlan>,
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

    debug_assert_eq!(active.kind, CommandPlanKind::HoldLine);
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

            let expires_at = match active.expiry_duration_ticks {
                Some(duration) => {
                    let Some(expires_at) = clock.tick.checked_add(duration) else {
                        warn!("discarded tactical plan with overflowing expiry");
                        placement.cancel();
                        return;
                    };
                    Some(expires_at)
                }
                None => None,
            };
            let id = ids.allocate();
            let plan = hold_line_plan(
                id,
                labels.next_label(CommandPlanKind::HoldLine),
                from_m,
                to_m,
                point_m,
                expires_at,
                clock.tick,
            );

            if let Err(error) = plan.validate() {
                warn!(?error, "discarded invalid tactical plan placement");
                placement.cancel();
                return;
            }

            let entity = commands
                .spawn((
                    MissionScoped,
                    Name::new(plan.label.clone()),
                    CommandPlanAssignees::default(),
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
    id: crate::gameplay::command_plans::CommandPlanId,
    label: String,
    from_m: Vec2,
    to_m: Vec2,
    rally_point_m: Vec2,
    expires_at: Option<u64>,
    created_tick: u64,
) -> CommandPlan {
    CommandPlan {
        id,
        label,
        kind: CommandPlanKind::HoldLine,
        area: CommandPlanArea::Line { from_m, to_m },
        rally_point_m,
        expires_at,
        created_tick,
    }
}

fn refresh_selected_plan(mut selected: ResMut<SelectedPlan>, plans: Query<(), With<CommandPlan>>) {
    if selected
        .entity
        .is_some_and(|entity| plans.get(entity).is_err())
    {
        selected.entity = None;
        selected.preview = false;
        selected.assignment_mode = false;
    }
}

fn sync_plan_active_action(
    placement: Res<PlanPlacementState>,
    selected: Res<SelectedPlan>,
    mut action: ResMut<ActiveAction>,
) {
    if let Some(placement) = placement.active.as_ref() {
        action.set(placement.instruction());
    } else if selected.assignment_mode {
        action.set("Assign Plan: Select Squad Leader");
    } else {
        action.clear();
    }
}

fn reset_plan_ui_state(
    mut placement: ResMut<PlanPlacementState>,
    mut selected: ResMut<SelectedPlan>,
    mut labels: ResMut<PlanLabelAllocator>,
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
    fn zero_expiry_minutes_means_no_expiration() {
        assert_eq!(expiry_duration_ticks(0), Ok(None));
        assert_eq!(
            expiry_duration_ticks(5),
            Ok(Some(5 * SIMULATION_TICKS_PER_MINUTE))
        );
        assert!(expiry_duration_ticks(u64::MAX).is_err());
    }

    #[test]
    fn hold_line_placement_progresses_through_the_expected_instructions() {
        let mut placement = PlanPlacement::hold_line(Some(300));
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
            crate::gameplay::command_plans::CommandPlanId(0),
            "Hold Line 1".into(),
            Vec2::new(-5.0, 0.0),
            Vec2::new(5.0, 0.0),
            Vec2::new(0.0, -3.0),
            Some(42 + 5 * SIMULATION_TICKS_PER_MINUTE),
            42,
        );

        assert_eq!(plan.validate(), Ok(()));
        assert_eq!(plan.label, "Hold Line 1");
        assert_eq!(plan.expires_at, Some(42 + 5 * SIMULATION_TICKS_PER_MINUTE));
    }

    #[test]
    fn escape_cancels_assignment_mode_without_discarding_the_selected_plan() {
        let mut placement = PlanPlacementState::default();
        let mut selected = SelectedPlan {
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
    fn plan_ui_state_is_reset_on_mission_exit() {
        let mut placement = PlanPlacementState {
            active: Some(PlanPlacement::hold_line(None)),
        };
        let mut selected = SelectedPlan {
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
