#![doc = include_str!("../../docs/gameplay/command_plans.md")]

use crate::GameState;
use crate::actors::units::{Alive, Soldier};
use crate::ai::htn::trace::{DecisionTrace, TraceEvent};
use crate::gameplay::command::CommandForest;
use crate::gameplay::command_succession::CommandSuccessionDiagnostics;
use crate::gameplay::packets::{Address, Outbox, PacketIdAllocator, PacketPayload, SeenPackets};
use crate::gameplay::simulation::{SimulationClock, SimulationSet};
use crate::gameplay::spatial::PositionTarget;
use crate::gameplay::squads::{MemberOfSquad, Squad};
use bevy::prelude::*;

/// Tactical plans are persistent intent-bearing plans created during a
/// plan. They are deliberately distinct from authored `MissionDefinition`
/// data and never install concrete action orders directly.
pub struct CommandPlansPlugin;

impl Plugin for CommandPlansPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CommandPlanIdAllocator>()
            .add_message::<CommandPlanAssignmentRequested>()
            .add_systems(OnEnter(GameState::MissionScreen), reset_plan_id_allocator)
            .add_systems(
                Update,
                apply_plan_assignment_requests.run_if(in_state(GameState::MissionScreen)),
            )
            .add_systems(
                FixedUpdate,
                (
                    invalidate_delegation_on_revision,
                    transmit_pending_task_assignments,
                )
                    .chain()
                    .in_set(SimulationSet::Thinking)
                    .before(crate::ai::htn::synthesis::synthesize_beliefs)
                    .run_if(in_state(GameState::MissionScreen)),
            );
    }
}

/// Stable, mission-local identifier for a command plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandPlanId(pub u64);

/// Allocates monotonically increasing command-plan IDs within one mission.
#[derive(Resource, Debug, Default)]
pub struct CommandPlanIdAllocator {
    next: u64,
}

impl CommandPlanIdAllocator {
    pub fn allocate(&mut self) -> CommandPlanId {
        let id = CommandPlanId(self.next);
        self.next = self
            .next
            .checked_add(1)
            .expect("tactical plan ID allocator exhausted");
        id
    }

    pub fn reset(&mut self) {
        self.next = 0;
    }
}

fn reset_plan_id_allocator(mut allocator: ResMut<CommandPlanIdAllocator>) {
    allocator.reset();
}

/// Geometry defining where a command plan is to be executed. All positions
/// are in meters, not Bevy render units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CommandPlanArea {
    Line { from_m: Vec2, to_m: Vec2 },
    Point { center_m: Vec2 },
    Circle { center_m: Vec2, radius_m: f32 },
    Rect { min_m: Vec2, max_m: Vec2 },
}

impl CommandPlanArea {
    pub fn shape_name(&self) -> &'static str {
        match self {
            Self::Line { .. } => "line",
            Self::Point { .. } => "point",
            Self::Circle { .. } => "circle",
            Self::Rect { .. } => "rectangle",
        }
    }

    fn validate_geometry(&self) -> Result<(), CommandPlanValidationError> {
        match self {
            Self::Line { from_m, to_m } => {
                validate_finite_point("line start", *from_m)?;
                validate_finite_point("line end", *to_m)?;
                if from_m.distance_squared(*to_m) <= f32::EPSILON {
                    return Err(CommandPlanValidationError::DegenerateLine);
                }
            }
            Self::Point { center_m } => validate_finite_point("point center", *center_m)?,
            Self::Circle { center_m, radius_m } => {
                validate_finite_point("circle center", *center_m)?;
                if !radius_m.is_finite() {
                    return Err(CommandPlanValidationError::NonFiniteRadius);
                }
                if *radius_m <= 0.0 {
                    return Err(CommandPlanValidationError::NonPositiveRadius);
                }
            }
            Self::Rect { min_m, max_m } => {
                validate_finite_point("rectangle minimum", *min_m)?;
                validate_finite_point("rectangle maximum", *max_m)?;
                if min_m.x >= max_m.x || min_m.y >= max_m.y {
                    return Err(CommandPlanValidationError::DegenerateOrUnnormalizedRect);
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandPlanKind {
    HoldLine,
    SecurePerimeter,
    ScoutArea,
    ClearArea,
}

impl CommandPlanKind {
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::HoldLine => "Hold Line",
            Self::SecurePerimeter => "Secure Perimeter",
            Self::ScoutArea => "Scout Area",
            Self::ClearArea => "Clear Area",
        }
    }

    pub fn accepts_area(self, area: &CommandPlanArea) -> bool {
        matches!(
            (self, area),
            (Self::HoldLine, CommandPlanArea::Line { .. })
                | (
                    Self::SecurePerimeter,
                    CommandPlanArea::Circle { .. } | CommandPlanArea::Rect { .. }
                )
                | (
                    Self::ScoutArea,
                    CommandPlanArea::Point { .. }
                        | CommandPlanArea::Circle { .. }
                        | CommandPlanArea::Rect { .. }
                )
                | (
                    Self::ClearArea,
                    CommandPlanArea::Point { .. }
                        | CommandPlanArea::Circle { .. }
                        | CommandPlanArea::Rect { .. }
                )
        )
    }
}

/// Persistent player-authored command plan.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct CommandPlan {
    pub id: CommandPlanId,
    pub label: String,
    pub kind: CommandPlanKind,
    pub area: CommandPlanArea,
    pub rally_point_m: Vec2,
    pub expires_at: Option<u64>,
    pub created_tick: u64,
}

impl CommandPlan {
    pub fn validate(&self) -> Result<(), CommandPlanValidationError> {
        validate_plan_fields(
            &self.label,
            self.kind,
            &self.area,
            self.rally_point_m,
            self.expires_at,
            self.created_tick,
        )
    }

    pub fn snapshot(&self) -> CommandPlanSnapshot {
        CommandPlanSnapshot {
            id: self.id,
            label: self.label.clone(),
            kind: self.kind,
            area: self.area.clone(),
            rally_point_m: self.rally_point_m,
            expires_at: self.expires_at,
            created_tick: self.created_tick,
        }
    }
}

/// Persistent local record of which units the player assigned to a plan.
#[allow(dead_code)] // Constructed by milestone C and mutated by milestone D.
#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
pub struct CommandPlanAssignees {
    pub assignees: Vec<Entity>,
}

/// Copy of a command plan that is safe to transmit through the comms substrate.
/// It contains no local Bevy entity reference.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandPlanSnapshot {
    pub id: CommandPlanId,
    pub label: String,
    pub kind: CommandPlanKind,
    pub area: CommandPlanArea,
    pub rally_point_m: Vec2,
    pub expires_at: Option<u64>,
    pub created_tick: u64,
}

impl CommandPlanSnapshot {
    pub fn validate(&self) -> Result<(), CommandPlanValidationError> {
        validate_plan_fields(
            &self.label,
            self.kind,
            &self.area,
            self.rally_point_m,
            self.expires_at,
            self.created_tick,
        )
    }
}

/// CommandPlan intent installed on the receiving leader after a valid assignment
/// packet arrives. This is an HTN input, never a concrete action order.
#[allow(dead_code)] // Installed by milestone D packet handling.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct AssignedCommandPlan {
    pub plan: CommandPlanSnapshot,
    pub assigned_by: Entity,
    pub issued_tick: u64,
    pub received_tick: u64,
}

/// Packet-safe intent assignment sent from a commander to a plan recipient.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandPlanAssignmentMessage {
    pub plan: CommandPlanSnapshot,
    pub issued_tick: u64,
}

/// Requests a plan assignment from any UI or future AI commander. The
/// consumer validates authority and then either installs it locally or sends it
/// through the physical comms substrate.
#[derive(Message, Debug, Clone, Copy)]
pub struct CommandPlanAssignmentRequested {
    pub plan: Entity,
    pub issuer: Entity,
    pub assignee: Entity,
}

/// A subordinate planning directive. Like a plan, this is an HTN input and
/// never a concrete movement/combat order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskDirective {
    HoldStation {
        plan_id: CommandPlanId,
        station: PositionTarget,
        fallback: PositionTarget,
        expires_at: Option<u64>,
    },
}

impl TaskDirective {
    pub fn validate(self) -> Result<(), TaskValidationError> {
        match self {
            Self::HoldStation {
                station, fallback, ..
            } => {
                validate_position_target("hold station", station)?;
                validate_position_target("fallback station", fallback)?;
            }
        }
        Ok(())
    }

    pub fn plan_id(self) -> CommandPlanId {
        match self {
            Self::HoldStation { plan_id, .. } => plan_id,
        }
    }

    pub fn expires_at(self) -> Option<u64> {
        match self {
            Self::HoldStation { expires_at, .. } => expires_at,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskValidationError {
    NonFinitePoint(&'static str),
    NonFiniteFacing(&'static str),
}

fn validate_position_target(
    field: &'static str,
    target: PositionTarget,
) -> Result<(), TaskValidationError> {
    if !target.position_m.is_finite() {
        return Err(TaskValidationError::NonFinitePoint(field));
    }
    if target
        .heading_radians
        .is_some_and(|heading| !heading.is_finite())
    {
        return Err(TaskValidationError::NonFiniteFacing(field));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TaskAssignmentMessage {
    pub directive: TaskDirective,
    pub issued_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskCancellationMessage {
    pub plan_id: CommandPlanId,
    pub issued_tick: u64,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AssignedTask {
    pub directive: TaskDirective,
    pub assigned_by: Entity,
    pub issued_tick: u64,
    pub received_tick: u64,
}

pub fn should_install_task_assignment(
    current_issued_tick: Option<u64>,
    incoming: &TaskAssignmentMessage,
) -> bool {
    current_issued_tick.is_none_or(|current| incoming.issued_tick > current)
}

/// Intent emitted by a leader HTN operator and accepted by the communications
/// bridge on the following simulation pass.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct PendingTaskAssignment {
    pub plan_issued_tick: u64,
    pub squad_revision: u64,
    pub assignee: Entity,
    pub directive: TaskDirective,
}

/// Durable memory of delegation side effects. It prevents replanning from
/// retransmitting the same subordinate task every tick.
#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
pub struct CommandPlanDelegationProgress {
    pub plan: Option<(CommandPlanId, u64)>,
    pub squad_revision: Option<u64>,
    pub delegated_to: Vec<Entity>,
}

impl CommandPlanDelegationProgress {
    pub fn reset_for(&mut self, plan_id: CommandPlanId, issued_tick: u64, squad_revision: u64) {
        let identity = (plan_id, issued_tick);
        if self.plan != Some(identity) || self.squad_revision != Some(squad_revision) {
            self.plan = Some(identity);
            self.squad_revision = Some(squad_revision);
            self.delegated_to.clear();
        }
    }
}

fn plan_is_assignable(plan: &CommandPlan, tick: u64) -> bool {
    plan.validate().is_ok() && plan.expires_at.is_none_or(|expires_at| expires_at > tick)
}

fn apply_plan_assignment_requests(
    mut commands: Commands,
    mut requests: MessageReader<CommandPlanAssignmentRequested>,
    clock: Res<SimulationClock>,
    command_forest: Res<CommandForest>,
    mut packet_ids: ResMut<PacketIdAllocator>,
    mut plans: Query<(&CommandPlan, &mut CommandPlanAssignees)>,
    soldiers: Query<(), (With<Soldier>, With<Alive>)>,
    mut transmitters: Query<(&mut Outbox, &mut SeenPackets), (With<Soldier>, With<Alive>)>,
    assignments: Query<&AssignedCommandPlan>,
) {
    for request in requests.read().copied() {
        let Ok((plan, mut assignees)) = plans.get_mut(request.plan) else {
            warn!(?request, "plan assignment rejected: unknown plan");
            continue;
        };
        if !plan_is_assignable(plan, clock.tick)
            || !command_forest.can_issue_command(request.issuer, request.assignee, |entity| {
                soldiers.get(entity).is_ok()
            })
        {
            warn!(
                ?request,
                "plan assignment rejected: invalid plan, unit, or authority"
            );
            continue;
        }
        if command_forest.subordinates_of(request.assignee).is_empty() {
            warn!(
                ?request,
                "plan assignment rejected: assignee is not a squad leader"
            );
            continue;
        }
        if assignees.assignees.contains(&request.assignee) {
            continue;
        }

        let message = CommandPlanAssignmentMessage {
            plan: plan.snapshot(),
            issued_tick: clock.tick,
        };
        if request.issuer == request.assignee {
            if should_install_assignment(assignments.get(request.assignee).ok(), &message) {
                commands
                    .entity(request.assignee)
                    .insert(AssignedCommandPlan {
                        plan: message.plan,
                        assigned_by: request.issuer,
                        issued_tick: message.issued_tick,
                        received_tick: clock.tick,
                    });
            }
        } else {
            let Ok((mut outbox, mut seen)) = transmitters.get_mut(request.issuer) else {
                warn!(?request, "plan assignment rejected: issuer cannot transmit");
                continue;
            };
            outbox.send(
                &mut seen,
                &mut packet_ids,
                request.issuer,
                Address::Direct(request.assignee),
                clock.tick,
                PacketPayload::CommandPlanAssignment(message),
            );
        }
        assignees.assignees.push(request.assignee);
    }
}

fn invalidate_delegation_on_revision(
    mut commands: Commands,
    clock: Res<SimulationClock>,
    command_forest: Res<CommandForest>,
    mut packet_ids: ResMut<PacketIdAllocator>,
    mut diagnostics: ResMut<CommandSuccessionDiagnostics>,
    squads: Query<&Squad>,
    living_soldiers: Query<(), (With<Soldier>, With<Alive>)>,
    mut coordinators: Query<(
        Entity,
        &MemberOfSquad,
        &AssignedCommandPlan,
        &mut CommandPlanDelegationProgress,
        Option<&PendingTaskAssignment>,
        &mut Outbox,
        &mut SeenPackets,
        Option<&mut DecisionTrace>,
    )>,
) {
    for (coordinator, membership, assigned, mut progress, pending, mut outbox, mut seen, trace) in
        &mut coordinators
    {
        let Ok(squad) = squads.get(membership.squad) else {
            continue;
        };
        if squad.current_leader != Some(coordinator) {
            continue;
        }
        let identity = (assigned.plan.id, assigned.issued_tick);
        if progress.plan == Some(identity) && progress.squad_revision == Some(squad.revision) {
            continue;
        }

        let desired: Vec<_> = squad
            .members
            .iter()
            .copied()
            .filter(|member| *member != coordinator && living_soldiers.get(*member).is_ok())
            .collect();
        if let Some((old_plan_id, _)) = progress.plan {
            for obsolete in progress
                .delegated_to
                .iter()
                .copied()
                .filter(|recipient| !desired.contains(recipient))
            {
                if command_forest.can_issue_command(coordinator, obsolete, |entity| {
                    living_soldiers.get(entity).is_ok()
                }) {
                    outbox.send(
                        &mut seen,
                        &mut packet_ids,
                        coordinator,
                        Address::Direct(obsolete),
                        clock.tick,
                        PacketPayload::TaskCancellation(TaskCancellationMessage {
                            plan_id: old_plan_id,
                            issued_tick: clock.tick,
                        }),
                    );
                    diagnostics.cancellations_issued += 1;
                }
            }
        }

        diagnostics.redelegation_resets += 1;
        if let Some(mut trace) = trace {
            trace.push(
                clock.tick,
                clock.elapsed_s,
                TraceEvent::RedelegationReset {
                    plan_id: assigned.plan.id,
                    squad_revision: squad.revision,
                },
            );
        }
        progress.plan = Some(identity);
        progress.squad_revision = Some(squad.revision);
        progress.delegated_to.clear();
        if pending.is_some_and(|pending| pending.squad_revision != squad.revision) {
            commands
                .entity(coordinator)
                .remove::<PendingTaskAssignment>();
        }
    }
}

fn transmit_pending_task_assignments(
    mut commands: Commands,
    clock: Res<SimulationClock>,
    command_forest: Res<CommandForest>,
    mut packet_ids: ResMut<PacketIdAllocator>,
    living_soldiers: Query<(), (With<Soldier>, With<Alive>)>,
    squads: Query<&Squad>,
    assignments: Query<&AssignedCommandPlan>,
    memberships: Query<&MemberOfSquad>,
    mut leaders: Query<(
        Entity,
        &PendingTaskAssignment,
        &mut CommandPlanDelegationProgress,
        &mut Outbox,
        &mut SeenPackets,
    )>,
) {
    for (leader, pending, mut progress, mut outbox, mut seen) in &mut leaders {
        let plan_id = pending.directive.plan_id();
        let valid_context = assignments.get(leader).is_ok_and(|assigned| {
            assigned.plan.id == plan_id && assigned.issued_tick == pending.plan_issued_tick
        }) && memberships
            .get(leader)
            .ok()
            .and_then(|membership| squads.get(membership.squad).ok())
            .is_some_and(|squad| {
                squad.current_leader == Some(leader) && squad.revision == pending.squad_revision
            });
        if !valid_context {
            commands.entity(leader).remove::<PendingTaskAssignment>();
            continue;
        }
        progress.reset_for(plan_id, pending.plan_issued_tick, pending.squad_revision);

        if !progress.delegated_to.contains(&pending.assignee) {
            if pending.assignee == leader {
                commands.entity(leader).insert(AssignedTask {
                    directive: pending.directive,
                    assigned_by: leader,
                    issued_tick: clock.tick,
                    received_tick: clock.tick,
                });
            } else if command_forest.can_issue_command(leader, pending.assignee, |entity| {
                living_soldiers.get(entity).is_ok()
            }) {
                outbox.send(
                    &mut seen,
                    &mut packet_ids,
                    leader,
                    Address::Direct(pending.assignee),
                    clock.tick,
                    PacketPayload::TaskAssignment(TaskAssignmentMessage {
                        directive: pending.directive,
                        issued_tick: clock.tick,
                    }),
                );
            } else {
                warn!(
                    ?leader,
                    assignee = ?pending.assignee,
                    "task delegation skipped: recipient is dead or unauthorized"
                );
            }

            if pending.assignee == leader
                || command_forest.can_issue_command(leader, pending.assignee, |entity| {
                    living_soldiers.get(entity).is_ok()
                })
            {
                progress.delegated_to.push(pending.assignee);
            }
        }
        commands.entity(leader).remove::<PendingTaskAssignment>();
    }
}

/// True if an incoming assignment is newer than the one already installed.
pub fn should_install_assignment(
    current: Option<&AssignedCommandPlan>,
    incoming: &CommandPlanAssignmentMessage,
) -> bool {
    current.is_none_or(|current| incoming.issued_tick > current.issued_tick)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandPlanValidationError {
    EmptyLabel,
    IncompatibleArea {
        kind: CommandPlanKind,
        shape: &'static str,
    },
    NonFinitePoint {
        field: &'static str,
    },
    NonFiniteRadius,
    NonPositiveRadius,
    DegenerateLine,
    DegenerateOrUnnormalizedRect,
    ExpiryNotAfterCreation {
        expires_at: u64,
        created_tick: u64,
    },
}

fn validate_plan_fields(
    label: &str,
    kind: CommandPlanKind,
    area: &CommandPlanArea,
    rally_point_m: Vec2,
    expires_at: Option<u64>,
    created_tick: u64,
) -> Result<(), CommandPlanValidationError> {
    if label.trim().is_empty() {
        return Err(CommandPlanValidationError::EmptyLabel);
    }
    if !kind.accepts_area(area) {
        return Err(CommandPlanValidationError::IncompatibleArea {
            kind,
            shape: area.shape_name(),
        });
    }

    area.validate_geometry()?;
    validate_finite_point("rally point", rally_point_m)?;

    if let Some(expires_at) = expires_at
        && expires_at <= created_tick
    {
        return Err(CommandPlanValidationError::ExpiryNotAfterCreation {
            expires_at,
            created_tick,
        });
    }

    Ok(())
}

fn validate_finite_point(
    field: &'static str,
    point: Vec2,
) -> Result<(), CommandPlanValidationError> {
    if point.is_finite() {
        Ok(())
    } else {
        Err(CommandPlanValidationError::NonFinitePoint { field })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::units::{Rank, Role};
    use bevy::ecs::system::RunSystemOnce;

    fn hold_line() -> CommandPlan {
        CommandPlan {
            id: CommandPlanId(4),
            label: "Alpha line".into(),
            kind: CommandPlanKind::HoldLine,
            area: CommandPlanArea::Line {
                from_m: Vec2::new(-10.0, 0.0),
                to_m: Vec2::new(10.0, 0.0),
            },
            rally_point_m: Vec2::new(0.0, -5.0),
            expires_at: Some(101),
            created_tick: 100,
        }
    }

    #[test]
    fn valid_hold_line_plan_and_snapshot_validate() {
        let plan = hold_line();
        assert_eq!(plan.validate(), Ok(()));
        assert_eq!(plan.snapshot().validate(), Ok(()));
    }

    #[test]
    fn kinds_accept_only_supported_area_shapes() {
        let line = CommandPlanArea::Line {
            from_m: Vec2::ZERO,
            to_m: Vec2::X,
        };
        let point = CommandPlanArea::Point {
            center_m: Vec2::ZERO,
        };
        let circle = CommandPlanArea::Circle {
            center_m: Vec2::ZERO,
            radius_m: 1.0,
        };
        let rect = CommandPlanArea::Rect {
            min_m: Vec2::ZERO,
            max_m: Vec2::ONE,
        };

        assert!(CommandPlanKind::HoldLine.accepts_area(&line));
        assert!(!CommandPlanKind::HoldLine.accepts_area(&point));
        assert!(CommandPlanKind::SecurePerimeter.accepts_area(&circle));
        assert!(CommandPlanKind::SecurePerimeter.accepts_area(&rect));
        assert!(!CommandPlanKind::SecurePerimeter.accepts_area(&point));
        assert!(CommandPlanKind::ScoutArea.accepts_area(&point));
        assert!(CommandPlanKind::ClearArea.accepts_area(&rect));
    }

    #[test]
    fn validation_rejects_invalid_geometry_and_metadata() {
        let mut plan = hold_line();
        plan.area = CommandPlanArea::Line {
            from_m: Vec2::ZERO,
            to_m: Vec2::ZERO,
        };
        assert_eq!(
            plan.validate(),
            Err(CommandPlanValidationError::DegenerateLine)
        );

        plan = hold_line();
        plan.area = CommandPlanArea::Circle {
            center_m: Vec2::ZERO,
            radius_m: 0.0,
        };
        plan.kind = CommandPlanKind::SecurePerimeter;
        assert_eq!(
            plan.validate(),
            Err(CommandPlanValidationError::NonPositiveRadius)
        );

        plan = hold_line();
        plan.area = CommandPlanArea::Rect {
            min_m: Vec2::new(2.0, 0.0),
            max_m: Vec2::new(1.0, 1.0),
        };
        plan.kind = CommandPlanKind::ScoutArea;
        assert_eq!(
            plan.validate(),
            Err(CommandPlanValidationError::DegenerateOrUnnormalizedRect)
        );

        plan = hold_line();
        plan.rally_point_m = Vec2::new(f32::NAN, 0.0);
        assert_eq!(
            plan.validate(),
            Err(CommandPlanValidationError::NonFinitePoint {
                field: "rally point"
            })
        );

        plan = hold_line();
        plan.expires_at = Some(plan.created_tick);
        assert_eq!(
            plan.validate(),
            Err(CommandPlanValidationError::ExpiryNotAfterCreation {
                expires_at: 100,
                created_tick: 100,
            })
        );
    }

    #[test]
    fn validation_rejects_empty_labels_and_incompatible_areas() {
        let mut plan = hold_line();
        plan.label = " \t".into();
        assert_eq!(plan.validate(), Err(CommandPlanValidationError::EmptyLabel));

        plan = hold_line();
        plan.area = CommandPlanArea::Point {
            center_m: Vec2::ZERO,
        };
        assert_eq!(
            plan.validate(),
            Err(CommandPlanValidationError::IncompatibleArea {
                kind: CommandPlanKind::HoldLine,
                shape: "point",
            })
        );
    }

    #[test]
    fn plan_cannot_be_assigned_at_or_after_expiry() {
        let plan = hold_line();
        assert!(plan_is_assignable(&plan, 100));
        assert!(!plan_is_assignable(&plan, 101));
        assert!(!plan_is_assignable(&plan, 102));

        let mut no_expiry = plan;
        no_expiry.expires_at = None;
        assert!(plan_is_assignable(&no_expiry, u64::MAX));
    }

    #[test]
    fn newer_assignment_supersedes_but_equal_or_older_assignments_do_not() {
        let plan = hold_line().snapshot();
        let incoming = CommandPlanAssignmentMessage {
            plan: plan.clone(),
            issued_tick: 11,
        };
        let current = AssignedCommandPlan {
            plan,
            assigned_by: Entity::PLACEHOLDER,
            issued_tick: 10,
            received_tick: 10,
        };

        assert!(should_install_assignment(None, &incoming));
        assert!(should_install_assignment(Some(&current), &incoming));
        assert!(!should_install_assignment(
            Some(&current),
            &CommandPlanAssignmentMessage {
                plan: current.plan.clone(),
                issued_tick: 10,
            }
        ));
    }

    #[test]
    fn task_validation_rejects_non_finite_directive_fields() {
        let invalid_station = TaskDirective::HoldStation {
            plan_id: CommandPlanId(1),
            station: PositionTarget::new(Vec2::new(f32::NAN, 0.0), None),
            fallback: PositionTarget::new(Vec2::ZERO, None),
            expires_at: None,
        };
        assert_eq!(
            invalid_station.validate(),
            Err(TaskValidationError::NonFinitePoint("hold station"))
        );

        let invalid_fallback = TaskDirective::HoldStation {
            plan_id: CommandPlanId(1),
            station: PositionTarget::new(Vec2::ZERO, None),
            fallback: PositionTarget::new(Vec2::new(f32::INFINITY, 0.0), None),
            expires_at: None,
        };
        assert_eq!(
            invalid_fallback.validate(),
            Err(TaskValidationError::NonFinitePoint("fallback station"))
        );

        let invalid_facing = TaskDirective::HoldStation {
            plan_id: CommandPlanId(1),
            station: PositionTarget::new(Vec2::ZERO, Some(f32::INFINITY)),
            fallback: PositionTarget::new(Vec2::ZERO, None),
            expires_at: None,
        };
        assert_eq!(
            invalid_facing.validate(),
            Err(TaskValidationError::NonFiniteFacing("hold station"))
        );
    }

    #[test]
    fn delegation_bridge_sends_task_and_records_progress() {
        let mut world = World::new();
        world.init_resource::<SimulationClock>();
        world.init_resource::<PacketIdAllocator>();
        let subordinate = world
            .spawn((
                Soldier {
                    rank: Rank::Private,
                    role: Role::Rifleman,
                },
                Alive,
            ))
            .id();
        let directive = TaskDirective::HoldStation {
            plan_id: CommandPlanId(7),
            station: PositionTarget::new(Vec2::X, Some(0.5)),
            fallback: PositionTarget::new(Vec2::NEG_Y, Some(0.5)),
            expires_at: None,
        };
        let leader = world
            .spawn((
                Soldier {
                    rank: Rank::Sergeant,
                    role: Role::Rifleman,
                },
                Alive,
                PendingTaskAssignment {
                    plan_issued_tick: 3,
                    squad_revision: 0,
                    assignee: subordinate,
                    directive,
                },
                CommandPlanDelegationProgress::default(),
                Outbox::default(),
                SeenPackets::default(),
            ))
            .id();
        let mut plan = hold_line();
        plan.id = CommandPlanId(7);
        world.entity_mut(leader).insert(AssignedCommandPlan {
            plan: plan.snapshot(),
            assigned_by: leader,
            issued_tick: 3,
            received_tick: 3,
        });
        let squad = world
            .spawn(Squad {
                id: crate::gameplay::squads::SquadId("test"),
                label: "Test",
                side: crate::actors::units::Side::Blue,
                members: vec![leader, subordinate],
                current_leader: Some(leader),
                revision: 0,
            })
            .id();
        world.entity_mut(leader).insert(MemberOfSquad {
            squad,
            roster_index: 0,
        });
        let mut forest = CommandForest::default();
        forest.set_superior(subordinate, Some(leader)).unwrap();
        world.insert_resource(forest);

        world
            .run_system_once(transmit_pending_task_assignments)
            .unwrap();
        world.flush();

        let progress = world.get::<CommandPlanDelegationProgress>(leader).unwrap();
        assert_eq!(progress.plan, Some((CommandPlanId(7), 3)));
        assert_eq!(progress.delegated_to, vec![subordinate]);
        assert!(world.get::<PendingTaskAssignment>(leader).is_none());
        let outbox = world.get::<Outbox>(leader).unwrap();
        assert!(matches!(
            outbox.packets[0].payload,
            PacketPayload::TaskAssignment(TaskAssignmentMessage {
                directive: sent,
                ..
            }) if sent == directive
        ));
    }

    #[test]
    fn allocator_is_monotonic_and_resettable() {
        let mut allocator = CommandPlanIdAllocator::default();
        assert_eq!(allocator.allocate(), CommandPlanId(0));
        assert_eq!(allocator.allocate(), CommandPlanId(1));
        allocator.reset();
        assert_eq!(allocator.allocate(), CommandPlanId(0));
    }

    #[test]
    fn squad_revision_mismatch_clears_all_progress_and_cancels_obsolete_recipient() {
        let mut world = World::new();
        world.insert_resource(SimulationClock {
            tick: 20,
            ..default()
        });
        world.init_resource::<PacketIdAllocator>();
        world.init_resource::<CommandSuccessionDiagnostics>();
        let leader = world
            .spawn((
                Soldier {
                    rank: Rank::Sergeant,
                    role: Role::Rifleman,
                },
                Alive,
                Outbox::default(),
                SeenPackets::default(),
            ))
            .id();
        let current = world
            .spawn((
                Soldier {
                    rank: Rank::Private,
                    role: Role::Rifleman,
                },
                Alive,
            ))
            .id();
        let obsolete = world
            .spawn((
                Soldier {
                    rank: Rank::Private,
                    role: Role::Rifleman,
                },
                Alive,
            ))
            .id();
        let squad = world
            .spawn(Squad {
                id: crate::gameplay::squads::SquadId("test"),
                label: "Test",
                side: crate::actors::units::Side::Blue,
                members: vec![leader, current],
                current_leader: Some(leader),
                revision: 2,
            })
            .id();
        world.entity_mut(leader).insert((
            MemberOfSquad {
                squad,
                roster_index: 0,
            },
            AssignedCommandPlan {
                plan: hold_line().snapshot(),
                assigned_by: leader,
                issued_tick: 10,
                received_tick: 10,
            },
            CommandPlanDelegationProgress {
                plan: Some((CommandPlanId(4), 10)),
                squad_revision: Some(1),
                delegated_to: vec![current, obsolete],
            },
            PendingTaskAssignment {
                plan_issued_tick: 10,
                squad_revision: 1,
                assignee: current,
                directive: TaskDirective::HoldStation {
                    plan_id: CommandPlanId(4),
                    station: PositionTarget::new(Vec2::X, None),
                    fallback: PositionTarget::new(Vec2::ZERO, None),
                    expires_at: None,
                },
            },
        ));
        let mut forest = CommandForest::default();
        forest.set_superior(current, Some(leader)).unwrap();
        forest.set_superior(obsolete, Some(leader)).unwrap();
        world.insert_resource(forest);

        world
            .run_system_once(invalidate_delegation_on_revision)
            .unwrap();
        world.flush();

        let progress = world.get::<CommandPlanDelegationProgress>(leader).unwrap();
        assert_eq!(progress.squad_revision, Some(2));
        assert!(progress.delegated_to.is_empty());
        assert!(world.get::<PendingTaskAssignment>(leader).is_none());
        let outbox = world.get::<Outbox>(leader).unwrap();
        assert_eq!(outbox.packets.len(), 1);
        assert!(matches!(
            outbox.packets[0].payload,
            PacketPayload::TaskCancellation(TaskCancellationMessage {
                plan_id: CommandPlanId(4),
                issued_tick: 20,
            })
        ));
        let diagnostics = world.resource::<CommandSuccessionDiagnostics>();
        assert_eq!(diagnostics.redelegation_resets, 1);
        assert_eq!(diagnostics.cancellations_issued, 1);
    }

    #[test]
    fn entering_a_plan_resets_the_plan_id_allocator() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<GameState>()
            .init_resource::<CommandForest>()
            .init_resource::<PacketIdAllocator>()
            .init_resource::<SimulationClock>()
            .add_plugins(CommandPlansPlugin);

        {
            let mut allocator = app.world_mut().resource_mut::<CommandPlanIdAllocator>();
            assert_eq!(allocator.allocate(), CommandPlanId(0));
            assert_eq!(allocator.allocate(), CommandPlanId(1));
        }

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::MissionScreen);
        app.update();

        assert_eq!(
            app.world_mut()
                .resource_mut::<CommandPlanIdAllocator>()
                .allocate(),
            CommandPlanId(0)
        );
    }
}
