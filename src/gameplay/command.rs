#![doc = include_str!("../../docs/gameplay/command.md")]

use crate::units::Side;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

pub struct CommandPlugin;

impl Plugin for CommandPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CommandForest>();
    }
}

/// Stable data-facing identifier for a unit.
///
/// Mission/external definitions should use this instead of Bevy `Entity`, because
/// entities are runtime-only IDs assigned when the mission is spawned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnitId(pub &'static str);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnitIdentity {
    pub id: UnitId,
}

/// Data-facing command assignment loaded before entities exist.
#[derive(Debug, Clone, Copy)]
pub struct CommandAssignmentDefinition {
    pub subordinate: UnitId,
    pub superior: Option<UnitId>,
}

/// Runtime individual-level command forest.
///
/// This is intentionally a forest rather than a single tree: each side may have
/// separate roots, and restructuring/elimination can temporarily orphan subtrees.
#[derive(Resource, Debug, Default)]
pub struct CommandForest {
    superior_of: HashMap<Entity, Entity>,
    subordinates_of: HashMap<Entity, Vec<Entity>>,
}

#[allow(dead_code)]
impl CommandForest {
    pub fn from_assignments(
        assignments: &[CommandAssignmentDefinition],
        entities_by_unit_id: &HashMap<UnitId, Entity>,
        side_of: impl Fn(Entity) -> Option<Side>,
    ) -> Self {
        let mut forest = Self::default();

        for assignment in assignments {
            let Some(&subordinate) = entities_by_unit_id.get(&assignment.subordinate) else {
                warn!(
                    "Command assignment skipped: unknown subordinate unit id {:?}",
                    assignment.subordinate
                );
                continue;
            };

            let Some(superior_id) = assignment.superior else {
                forest.ensure_node(subordinate);
                continue;
            };

            let Some(&superior) = entities_by_unit_id.get(&superior_id) else {
                warn!(
                    "Command assignment skipped: unknown superior unit id {:?}",
                    superior_id
                );
                forest.ensure_node(subordinate);
                continue;
            };

            if side_of(subordinate) != side_of(superior) {
                warn!(
                    "Command assignment skipped: subordinate {:?} and superior {:?} have different sides",
                    assignment.subordinate, superior_id
                );
                forest.ensure_node(subordinate);
                forest.ensure_node(superior);
                continue;
            }

            if let Err(error) = forest.set_superior(subordinate, Some(superior)) {
                warn!(
                    "Command assignment skipped for subordinate {:?}: {}",
                    assignment.subordinate, error
                );
            }
        }

        forest
    }

    pub fn ensure_node(&mut self, entity: Entity) {
        self.subordinates_of.entry(entity).or_default();
    }

    pub fn superior_of(&self, entity: Entity) -> Option<Entity> {
        self.superior_of.get(&entity).copied()
    }

    pub fn subordinates_of(&self, entity: Entity) -> &[Entity] {
        self.subordinates_of
            .get(&entity)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn roots(&self) -> Vec<Entity> {
        self.subordinates_of
            .keys()
            .copied()
            .filter(|entity| !self.superior_of.contains_key(entity))
            .collect()
    }

    pub fn descendants_of(&self, entity: Entity) -> Vec<Entity> {
        let mut descendants = Vec::new();
        let mut frontier = self.subordinates_of(entity).to_vec();

        while let Some(current) = frontier.pop() {
            descendants.push(current);
            frontier.extend_from_slice(self.subordinates_of(current));
        }

        descendants
    }

    /// True when `superior` is in `subordinate`'s ancestor chain.
    pub fn is_superior_of(&self, superior: Entity, subordinate: Entity) -> bool {
        let mut current = subordinate;
        let mut visited = HashSet::new();

        while let Some(parent) = self.superior_of(current) {
            if !visited.insert(current) {
                return false;
            }

            if parent == superior {
                return true;
            }

            current = parent;
        }

        false
    }

    /// Authority check for issuing orders through the command forest.
    ///
    /// Self-command is allowed so the player-controlled commander can receive a
    /// direct order. For subordinate units, the issuer must be an ancestor.
    pub fn can_command(&self, issuer: Entity, recipient: Entity) -> bool {
        issuer == recipient || self.is_superior_of(issuer, recipient)
    }

    pub fn set_superior(
        &mut self,
        subordinate: Entity,
        superior: Option<Entity>,
    ) -> Result<(), &'static str> {
        self.ensure_node(subordinate);

        if let Some(superior) = superior {
            if subordinate == superior {
                return Err("unit cannot command itself");
            }

            self.ensure_node(superior);

            if self.is_superior_of(subordinate, superior) {
                return Err("assignment would create a command cycle");
            }
        }

        if let Some(old_superior) = self.superior_of.remove(&subordinate) {
            if let Some(children) = self.subordinates_of.get_mut(&old_superior) {
                children.retain(|child| *child != subordinate);
            }
        }

        if let Some(superior) = superior {
            self.superior_of.insert(subordinate, superior);
            let children = self.subordinates_of.entry(superior).or_default();
            if !children.contains(&subordinate) {
                children.push(subordinate);
            }
        }

        Ok(())
    }

    /// Remove a unit from the forest. Current conservative policy: direct
    /// subordinates become roots rather than being auto-reattached.
    pub fn remove_unit(&mut self, entity: Entity) {
        if let Some(superior) = self.superior_of.remove(&entity) {
            if let Some(children) = self.subordinates_of.get_mut(&superior) {
                children.retain(|child| *child != entity);
            }
        }

        if let Some(children) = self.subordinates_of.remove(&entity) {
            for child in children {
                self.superior_of.remove(&child);
                self.ensure_node(child);
            }
        }
    }
}
