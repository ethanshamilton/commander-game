use crate::ai::perception::ContactKind;
use crate::gameplay::missions::{MissionArea, MissionId, MissionKind};
use crate::gameplay::spatial::PositionTarget;
use bevy::prelude::*;

/// Compact planning snapshot synthesized from a unit's own memory and components.
///
/// Despite containing Bevy entity handles, this is not ground-truth simulation
/// state. It is the planner's local view: what this actor currently believes is
/// decision-relevant.
#[derive(Debug, Clone)]
pub struct PlannerState {
    pub position_m: Vec2,
    pub heading_radians: f32,
    pub health_frac: f32,
    pub has_ammo: bool,
    pub nearest_hostile: Option<HostileBelief>,
    pub under_fire: bool,
    pub has_move_target: bool,
    pub tick: u64,
    pub assigned_mission: Option<AssignedMissionBelief>,
    pub next_hold_station: Option<HoldStationAssignment>,
    pub mission_delegation_complete: bool,
    pub delegated_assignees: Vec<Entity>,
    pub has_command_responsibility: bool,
    pub own_mission_target: Option<PositionTarget>,
    pub own_fallback_target: Option<PositionTarget>,
    pub at_own_mission_target: bool,
    pub assigned_task: Option<AssignedTaskBelief>,
    pub at_assigned_station: bool,
    /// Pose from the newest expired mission/task assignment.
    pub fallback_target: Option<PositionTarget>,
    pub at_fallback_target: bool,
}

impl PlannerState {
    pub fn hostile_staleness_ticks(&self) -> Option<u64> {
        self.nearest_hostile
            .map(|hostile| self.tick.saturating_sub(hostile.last_seen_tick))
    }

    pub fn hostile_is_fresh(&self, max_staleness_ticks: u64) -> bool {
        self.hostile_staleness_ticks()
            .is_some_and(|staleness| staleness <= max_staleness_ticks)
    }

    pub fn mission_is_expired(&self) -> bool {
        self.assigned_mission
            .and_then(|mission| mission.expires_at)
            .is_some_and(|expires_at| self.tick >= expires_at)
    }

    pub fn has_delegated_to(&self, mission: (MissionId, u64), assignee: Entity) -> bool {
        self.assigned_mission
            .is_some_and(|assigned| assigned.identity() == mission)
            && self.delegated_assignees.contains(&assignee)
    }

    pub fn assigned_task_is_expired(&self) -> bool {
        self.assigned_task
            .and_then(|task| task.expires_at)
            .is_some_and(|expires_at| self.tick >= expires_at)
    }

    pub fn fallback_is_active(&self) -> bool {
        self.fallback_target.is_some()
    }
}

/// Mission facts projected from the recipient's `AssignedMission` component.
/// This is the planner-facing view, not a second operational source of truth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AssignedMissionBelief {
    pub id: MissionId,
    pub issued_tick: u64,
    pub kind: MissionKind,
    pub area: MissionArea,
    pub rally_point_m: Vec2,
    pub expires_at: Option<u64>,
}

impl AssignedMissionBelief {
    pub fn identity(self) -> (MissionId, u64) {
        (self.id, self.issued_tick)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedTaskKind {
    HoldStation,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AssignedTaskBelief {
    pub mission_id: MissionId,
    pub issued_tick: u64,
    pub kind: AssignedTaskKind,
    pub station: PositionTarget,
    pub fallback: PositionTarget,
    pub expires_at: Option<u64>,
}

impl AssignedTaskBelief {
    pub fn identity(self) -> (MissionId, u64) {
        (self.mission_id, self.issued_tick)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoldStationAssignment {
    pub assignee: Entity,
    pub station: PositionTarget,
    pub fallback: PositionTarget,
}

#[derive(Debug, Clone, Copy)]
pub struct HostileBelief {
    pub entity: Entity,
    pub position_m: Vec2,
    pub confidence: f32,
    pub last_seen_tick: u64,
    pub kind: ContactKind,
}

impl Default for PlannerState {
    fn default() -> Self {
        Self {
            position_m: Vec2::ZERO,
            heading_radians: 0.0,
            health_frac: 1.0,
            has_ammo: false,
            nearest_hostile: None,
            under_fire: false,
            has_move_target: false,
            tick: 0,
            assigned_mission: None,
            next_hold_station: None,
            mission_delegation_complete: false,
            delegated_assignees: Vec::new(),
            has_command_responsibility: false,
            own_mission_target: None,
            own_fallback_target: None,
            at_own_mission_target: false,
            assigned_task: None,
            at_assigned_station: false,
            fallback_target: None,
            at_fallback_target: false,
        }
    }
}

/// Quantized projection of `PlannerState` used to detect decision-relevant change.
/// The executor replans only when this digest changes.
///
/// CONTRACT: when adding a field to `PlannerState`, decide HERE whether it is
/// decision-relevant. If any domain precondition reads the new field, the digest
/// must reflect it (band/bool-quantized, never raw floats or ticks), or units will
/// silently fail to replan when it changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlannerStateDigest {
    pub nearest_hostile: Option<Entity>,
    pub hostile_fresh: bool,
    pub health_band: u8,
    pub has_ammo: bool,
    pub under_fire: bool,
    pub has_move_target: bool,
    pub assigned_mission: Option<(MissionId, u64)>,
    pub mission_expired: bool,
    pub next_hold_station_assignee: Option<Entity>,
    pub next_hold_station_dm: Option<(i32, i32)>,
    pub mission_delegation_complete: bool,
    pub has_command_responsibility: bool,
    pub own_mission_target: Option<PositionTargetDigest>,
    pub at_own_mission_target: bool,
    pub assigned_task: Option<(MissionId, u64)>,
    pub assigned_task_expired: bool,
    pub at_assigned_station: bool,
    pub fallback_target: Option<PositionTargetDigest>,
    pub at_fallback_target: bool,
}

impl PlannerStateDigest {
    pub fn from_state(state: &PlannerState) -> Self {
        Self {
            nearest_hostile: state.nearest_hostile.map(|hostile| hostile.entity),
            hostile_fresh: state.hostile_is_fresh(super::soldier::FRESH_CONTACT_TICKS),
            health_band: health_band(state.health_frac),
            has_ammo: state.has_ammo,
            under_fire: state.under_fire,
            has_move_target: state.has_move_target,
            assigned_mission: state.assigned_mission.map(AssignedMissionBelief::identity),
            mission_expired: state.mission_is_expired(),
            next_hold_station_assignee: state.next_hold_station.map(|next| next.assignee),
            next_hold_station_dm: state
                .next_hold_station
                .map(|next| quantize_decimeters(next.station.position_m)),
            mission_delegation_complete: state.mission_delegation_complete,
            has_command_responsibility: state.has_command_responsibility,
            own_mission_target: state.own_mission_target.map(PositionTargetDigest::from),
            at_own_mission_target: state.at_own_mission_target,
            assigned_task: state.assigned_task.map(AssignedTaskBelief::identity),
            assigned_task_expired: state.assigned_task_is_expired(),
            at_assigned_station: state.at_assigned_station,
            fallback_target: state.fallback_target.map(PositionTargetDigest::from),
            at_fallback_target: state.at_fallback_target,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionTargetDigest {
    pub position_dm: (i32, i32),
    pub heading_centiradians: Option<i16>,
}

impl From<PositionTarget> for PositionTargetDigest {
    fn from(target: PositionTarget) -> Self {
        Self {
            position_dm: quantize_decimeters(target.position_m),
            heading_centiradians: target
                .heading_radians
                .map(|heading| (heading * 100.0).round() as i16),
        }
    }
}

fn quantize_decimeters(position_m: Vec2) -> (i32, i32) {
    (
        (position_m.x * 10.0).round() as i32,
        (position_m.y * 10.0).round() as i32,
    )
}

fn health_band(health_frac: f32) -> u8 {
    if health_frac < 0.35 {
        0
    } else if health_frac < 0.7 {
        1
    } else {
        2
    }
}
