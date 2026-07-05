use crate::ai::perception::ContactKind;
use bevy::prelude::*;

/// Compact planning snapshot synthesized from a unit's own memory and components.
///
/// Despite containing Bevy entity handles, this is not ground-truth simulation
/// state. It is the planner's local view: what this actor currently believes is
/// decision-relevant.
#[derive(Debug, Clone)]
pub struct PlannerState {
    pub position_m: Vec2,
    pub health_frac: f32,
    pub has_ammo: bool,
    pub nearest_hostile: Option<HostileBelief>,
    pub under_fire: bool,
    pub has_move_target: bool,
    pub tick: u64,
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
            health_frac: 1.0,
            has_ammo: false,
            nearest_hostile: None,
            under_fire: false,
            has_move_target: false,
            tick: 0,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannerStateDigest {
    pub nearest_hostile: Option<Entity>,
    pub hostile_fresh: bool,
    pub health_band: u8,
    pub has_ammo: bool,
    pub under_fire: bool,
    pub has_move_target: bool,
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
        }
    }
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
