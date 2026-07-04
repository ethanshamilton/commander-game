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
