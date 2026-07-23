//! Persistent squad organization.
//!
//! A squad roster answers who belongs to an organizational unit and, by its
//! authored order, who succeeds its leader. `CommandForest` remains the source
//! of current superior/subordinate authority.

use crate::actors::units::Side;
use crate::gameplay::command::UnitId;
use bevy::prelude::*;

/// Stable data-facing identifier for a squad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SquadId(pub &'static str);

/// Mission-authored squad. The first member is the initial leader and each
/// following member is the next candidate in command succession.
#[derive(Debug, Clone, Copy)]
pub struct SquadDefinition {
    pub id: SquadId,
    pub label: &'static str,
    pub members: &'static [UnitId],
}

/// Runtime organizational truth for one squad.
///
/// `members` keeps its authored order when leadership changes. Promotion
/// changes `current_leader`, never the roster itself.
#[allow(dead_code)] // Identity/revision are consumed by the succession slice.
#[derive(Component, Debug, Clone)]
pub struct Squad {
    pub id: SquadId,
    pub label: &'static str,
    pub side: Side,
    pub members: Vec<Entity>,
    pub current_leader: Option<Entity>,
    pub revision: u64,
}

#[allow(dead_code)] // Successor lookup is consumed by the next command-succession slice.
impl Squad {
    pub fn roster_index(&self, unit: Entity) -> Option<usize> {
        self.members.iter().position(|member| *member == unit)
    }

    /// Select the first eligible member after the current leader in authored
    /// roster order. Eligibility (alive, capable, same side) belongs to the
    /// caller because it depends on runtime ECS state.
    pub fn next_successor(&self, mut eligible: impl FnMut(Entity) -> bool) -> Option<Entity> {
        let start = self
            .current_leader
            .and_then(|leader| self.roster_index(leader))
            .map_or(0, |index| index + 1);

        self.members[start..]
            .iter()
            .copied()
            .find(|member| eligible(*member))
    }
}

/// Reverse lookup installed on every squad member.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemberOfSquad {
    pub squad: Entity,
    pub roster_index: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successor_uses_roster_order_and_skips_ineligible_members() {
        let members = [
            Entity::from_raw_u32(1).unwrap(),
            Entity::from_raw_u32(2).unwrap(),
            Entity::from_raw_u32(3).unwrap(),
            Entity::from_raw_u32(4).unwrap(),
        ];
        let squad = Squad {
            id: SquadId("alpha"),
            label: "Alpha",
            side: Side::Blue,
            members: members.to_vec(),
            current_leader: Some(members[0]),
            revision: 0,
        };

        assert_eq!(
            squad.next_successor(|member| member != members[1]),
            Some(members[2])
        );
    }

    #[test]
    fn promotion_does_not_reconsider_predecessors() {
        let members = [
            Entity::from_raw_u32(1).unwrap(),
            Entity::from_raw_u32(2).unwrap(),
            Entity::from_raw_u32(3).unwrap(),
        ];
        let squad = Squad {
            id: SquadId("alpha"),
            label: "Alpha",
            side: Side::Blue,
            members: members.to_vec(),
            current_leader: Some(members[1]),
            revision: 1,
        };

        assert_eq!(squad.next_successor(|_| true), Some(members[2]));
    }
}
