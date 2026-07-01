#![doc = include_str!("../../docs/gameplay/command.md")]

use crate::actors::units::Side;
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::world::World;

    fn spawn_entities(count: usize) -> (World, Vec<Entity>) {
        let mut world = World::new();
        let entities = (0..count).map(|_| world.spawn_empty().id()).collect();
        (world, entities)
    }

    /// The forest is a denormalized pair of maps; every mutation must keep
    /// them coherent. One helper catches a whole family of desync bugs.
    fn assert_coherent(forest: &CommandForest) {
        for (&child, &parent) in &forest.superior_of {
            assert!(
                forest.subordinates_of(parent).contains(&child),
                "superior_of has {child:?} -> {parent:?} but parent's subordinate list disagrees"
            );
        }
        for (&parent, children) in &forest.subordinates_of {
            for &child in children {
                assert_eq!(
                    forest.superior_of(child),
                    Some(parent),
                    "{parent:?} lists {child:?} as subordinate but superior_of disagrees"
                );
            }
        }
    }

    #[test]
    fn set_superior_rejects_cycles_and_self_command() {
        let (_world, e) = spawn_entities(3);
        let (a, b, c) = (e[0], e[1], e[2]);
        let mut forest = CommandForest::default();

        forest.set_superior(b, Some(a)).unwrap();
        forest.set_superior(c, Some(b)).unwrap();

        // closing the loop a -> b -> c -> a must fail, at any depth
        assert!(forest.set_superior(a, Some(c)).is_err());
        assert!(forest.set_superior(a, Some(b)).is_err());
        assert!(forest.set_superior(a, Some(a)).is_err());

        // failed assignments must not have mutated anything
        assert_eq!(forest.superior_of(a), None);
        assert_coherent(&forest);
    }

    #[test]
    fn reparenting_removes_child_from_old_superior() {
        let (_world, e) = spawn_entities(3);
        let (old, new, child) = (e[0], e[1], e[2]);
        let mut forest = CommandForest::default();

        forest.set_superior(child, Some(old)).unwrap();
        forest.set_superior(child, Some(new)).unwrap();

        assert_eq!(forest.superior_of(child), Some(new));
        assert!(
            !forest.subordinates_of(old).contains(&child),
            "stale subordinate entry left on old superior"
        );
        assert_coherent(&forest);

        // re-assigning to the same superior must not duplicate the child
        forest.set_superior(child, Some(new)).unwrap();
        assert_eq!(forest.subordinates_of(new).len(), 1);
    }

    #[test]
    fn remove_unit_orphans_children_as_roots_without_dangling_refs() {
        let (_world, e) = spawn_entities(4);
        let (root, mid, child_a, child_b) = (e[0], e[1], e[2], e[3]);
        let mut forest = CommandForest::default();

        forest.set_superior(mid, Some(root)).unwrap();
        forest.set_superior(child_a, Some(mid)).unwrap();
        forest.set_superior(child_b, Some(mid)).unwrap();

        forest.remove_unit(mid);

        // children become roots (conservative policy), not reattached to root
        assert_eq!(forest.superior_of(child_a), None);
        assert_eq!(forest.superior_of(child_b), None);
        assert!(!forest.subordinates_of(root).contains(&mid));
        assert!(forest.roots().contains(&child_a));
        assert_coherent(&forest);
    }

    #[test]
    fn can_command_allows_self_and_ancestors_only() {
        let (_world, e) = spawn_entities(4);
        let (root, mid, leaf, sibling) = (e[0], e[1], e[2], e[3]);
        let mut forest = CommandForest::default();

        forest.set_superior(mid, Some(root)).unwrap();
        forest.set_superior(leaf, Some(mid)).unwrap();
        forest.set_superior(sibling, Some(root)).unwrap();

        assert!(forest.can_command(leaf, leaf), "self-command allowed");
        assert!(forest.can_command(mid, leaf), "direct superior");
        assert!(forest.can_command(root, leaf), "transitive superior");
        assert!(!forest.can_command(leaf, mid), "authority is not upward");
        assert!(
            !forest.can_command(sibling, leaf),
            "authority is not lateral"
        );
    }

    #[test]
    fn from_assignments_skips_cross_side_links_but_keeps_units() {
        use crate::actors::units::Side;
        use std::collections::HashMap;

        let (_world, e) = spawn_entities(2);
        let (blue, red) = (e[0], e[1]);
        let ids: HashMap<UnitId, Entity> = [(UnitId("blue"), blue), (UnitId("red"), red)].into();

        let assignments = [CommandAssignmentDefinition {
            subordinate: UnitId("red"),
            superior: Some(UnitId("blue")),
        }];

        let forest = CommandForest::from_assignments(&assignments, &ids, |entity| {
            Some(if entity == blue {
                Side::Blue
            } else {
                Side::Red
            })
        });

        // link rejected, but both units still exist as roots
        assert_eq!(forest.superior_of(red), None);
        let roots = forest.roots();
        assert!(roots.contains(&red) && roots.contains(&blue));
        assert_coherent(&forest);
    }
}
