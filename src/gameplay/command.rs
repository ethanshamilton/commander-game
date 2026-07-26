#![doc = include_str!("../../docs/gameplay/command.md")]

use crate::actors::units::Side;
use crate::gameplay::command_succession::register_command_succession;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

pub struct CommandPlugin;

impl Plugin for CommandPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CommandForest>();
        register_command_succession(app);
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
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct CommandForest {
    superior_of: HashMap<Entity, Entity>,
    subordinates_of: HashMap<Entity, Vec<Entity>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessionOutcome {
    pub deceased: Entity,
    pub old_superior: Option<Entity>,
    pub successor: Option<Entity>,
    pub transferred_subordinates: Vec<Entity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandMutationError {
    UnknownUnit(Entity),
    InvalidSuccessor { deceased: Entity, successor: Entity },
    InvalidForest(CommandForestInvariantError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandForestInvariantError {
    MissingNode(Entity),
    MissingSubordinateEntry {
        parent: Entity,
        child: Entity,
    },
    IncorrectSuperior {
        parent: Entity,
        child: Entity,
        recorded: Option<Entity>,
    },
    DuplicateSubordinate {
        parent: Entity,
        child: Entity,
    },
    SelfCommand(Entity),
    Cycle(Entity),
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

    /// Topology-only authority check. This intentionally ignores life state.
    pub fn can_command_in_forest(&self, issuer: Entity, recipient: Entity) -> bool {
        issuer == recipient || self.is_superior_of(issuer, recipient)
    }

    /// Authority check for executable orders and assignments. Both endpoints
    /// must be living in addition to having a valid forest relationship.
    pub fn can_issue_command(
        &self,
        issuer: Entity,
        recipient: Entity,
        mut is_living: impl FnMut(Entity) -> bool,
    ) -> bool {
        is_living(issuer) && is_living(recipient) && self.can_command_in_forest(issuer, recipient)
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

    /// Atomically remove `deceased`, optionally promoting one of its direct
    /// children and transferring the other direct children beneath it.
    pub fn succeed(
        &mut self,
        deceased: Entity,
        successor: Option<Entity>,
    ) -> Result<SuccessionOutcome, CommandMutationError> {
        self.validate()
            .map_err(CommandMutationError::InvalidForest)?;
        if !self.subordinates_of.contains_key(&deceased) {
            return Err(CommandMutationError::UnknownUnit(deceased));
        }
        if let Some(successor) = successor
            && self.superior_of(successor) != Some(deceased)
        {
            return Err(CommandMutationError::InvalidSuccessor {
                deceased,
                successor,
            });
        }

        let old_superior = self.superior_of(deceased);
        let direct_children = self.subordinates_of(deceased).to_vec();
        let transferred_subordinates = successor.map_or_else(Vec::new, |successor| {
            direct_children
                .iter()
                .copied()
                .filter(|child| *child != successor)
                .collect::<Vec<_>>()
        });
        let mut prospective = self.clone();

        prospective.superior_of.remove(&deceased);
        prospective.subordinates_of.remove(&deceased);

        if let Some(parent) = old_superior {
            let children = prospective
                .subordinates_of
                .get_mut(&parent)
                .expect("validated parent must exist");
            if let Some(index) = children.iter().position(|child| *child == deceased) {
                if let Some(successor) = successor {
                    children[index] = successor;
                } else {
                    children.remove(index);
                }
            }
        }

        match successor {
            Some(successor) => {
                if let Some(parent) = old_superior {
                    prospective.superior_of.insert(successor, parent);
                } else {
                    prospective.superior_of.remove(&successor);
                }

                let successor_children = prospective
                    .subordinates_of
                    .get_mut(&successor)
                    .expect("validated successor must exist");
                for child in &transferred_subordinates {
                    prospective.superior_of.insert(*child, successor);
                    if !successor_children.contains(child) {
                        successor_children.push(*child);
                    }
                }
            }
            None => {
                for child in &direct_children {
                    prospective.superior_of.remove(child);
                }
            }
        }

        prospective
            .validate()
            .map_err(CommandMutationError::InvalidForest)?;
        *self = prospective;

        Ok(SuccessionOutcome {
            deceased,
            old_superior,
            successor,
            transferred_subordinates,
        })
    }

    pub fn validate(&self) -> Result<(), CommandForestInvariantError> {
        for (&child, &parent) in &self.superior_of {
            if child == parent {
                return Err(CommandForestInvariantError::SelfCommand(child));
            }
            if !self.subordinates_of.contains_key(&child) {
                return Err(CommandForestInvariantError::MissingNode(child));
            }
            let Some(children) = self.subordinates_of.get(&parent) else {
                return Err(CommandForestInvariantError::MissingNode(parent));
            };
            if !children.contains(&child) {
                return Err(CommandForestInvariantError::MissingSubordinateEntry { parent, child });
            }
        }

        for (&parent, children) in &self.subordinates_of {
            let mut seen = HashSet::new();
            for &child in children {
                if parent == child {
                    return Err(CommandForestInvariantError::SelfCommand(child));
                }
                if !self.subordinates_of.contains_key(&child) {
                    return Err(CommandForestInvariantError::MissingNode(child));
                }
                if !seen.insert(child) {
                    return Err(CommandForestInvariantError::DuplicateSubordinate {
                        parent,
                        child,
                    });
                }
                let recorded = self.superior_of(child);
                if recorded != Some(parent) {
                    return Err(CommandForestInvariantError::IncorrectSuperior {
                        parent,
                        child,
                        recorded,
                    });
                }
            }
        }

        for &start in self.subordinates_of.keys() {
            let mut current = start;
            let mut visited = HashSet::new();
            while let Some(parent) = self.superior_of(current) {
                if !visited.insert(current) {
                    return Err(CommandForestInvariantError::Cycle(current));
                }
                current = parent;
            }
        }

        Ok(())
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
    fn topology_authority_allows_self_and_ancestors_only() {
        let (_world, e) = spawn_entities(4);
        let (root, mid, leaf, sibling) = (e[0], e[1], e[2], e[3]);
        let mut forest = CommandForest::default();

        forest.set_superior(mid, Some(root)).unwrap();
        forest.set_superior(leaf, Some(mid)).unwrap();
        forest.set_superior(sibling, Some(root)).unwrap();

        assert!(
            forest.can_command_in_forest(leaf, leaf),
            "self-command allowed"
        );
        assert!(forest.can_command_in_forest(mid, leaf), "direct superior");
        assert!(
            forest.can_command_in_forest(root, leaf),
            "transitive superior"
        );
        assert!(
            !forest.can_command_in_forest(leaf, mid),
            "authority is not upward"
        );
        assert!(
            !forest.can_command_in_forest(sibling, leaf),
            "authority is not lateral"
        );
    }

    #[test]
    fn succession_rewrites_middle_node_and_preserves_order_and_subtrees() {
        let (_world, e) = spawn_entities(8);
        let (parent, before, deceased, after, successor, child_a, child_b, deep) =
            (e[0], e[1], e[2], e[3], e[4], e[5], e[6], e[7]);
        let mut forest = CommandForest::default();
        forest.set_superior(before, Some(parent)).unwrap();
        forest.set_superior(deceased, Some(parent)).unwrap();
        forest.set_superior(after, Some(parent)).unwrap();
        forest.set_superior(successor, Some(deceased)).unwrap();
        forest.set_superior(child_a, Some(deceased)).unwrap();
        forest.set_superior(child_b, Some(deceased)).unwrap();
        forest.set_superior(deep, Some(successor)).unwrap();

        let outcome = forest.succeed(deceased, Some(successor)).unwrap();

        assert_eq!(
            outcome,
            SuccessionOutcome {
                deceased,
                old_superior: Some(parent),
                successor: Some(successor),
                transferred_subordinates: vec![child_a, child_b],
            }
        );
        assert_eq!(forest.subordinates_of(parent), &[before, successor, after]);
        assert_eq!(forest.subordinates_of(successor), &[deep, child_a, child_b]);
        assert_eq!(forest.superior_of(successor), Some(parent));
        assert_eq!(forest.superior_of(child_a), Some(successor));
        assert!(!forest.subordinates_of.contains_key(&deceased));
        assert!(!forest.superior_of.contains_key(&deceased));
        assert_eq!(forest.validate(), Ok(()));
    }

    #[test]
    fn root_succession_promotes_a_new_root() {
        let (_world, e) = spawn_entities(3);
        let (deceased, successor, sibling) = (e[0], e[1], e[2]);
        let mut forest = CommandForest::default();
        forest.set_superior(successor, Some(deceased)).unwrap();
        forest.set_superior(sibling, Some(deceased)).unwrap();

        forest.succeed(deceased, Some(successor)).unwrap();

        assert_eq!(forest.superior_of(successor), None);
        assert_eq!(forest.superior_of(sibling), Some(successor));
        assert_eq!(forest.subordinates_of(successor), &[sibling]);
        assert!(forest.roots().contains(&successor));
        assert_eq!(forest.validate(), Ok(()));
    }

    #[test]
    fn succession_without_candidate_orphans_children_and_removes_leaf() {
        let (_world, e) = spawn_entities(4);
        let (parent, deceased, child_a, child_b) = (e[0], e[1], e[2], e[3]);
        let mut forest = CommandForest::default();
        forest.set_superior(deceased, Some(parent)).unwrap();
        forest.set_superior(child_a, Some(deceased)).unwrap();
        forest.set_superior(child_b, Some(deceased)).unwrap();

        let outcome = forest.succeed(deceased, None).unwrap();

        assert!(outcome.transferred_subordinates.is_empty());
        assert_eq!(forest.superior_of(child_a), None);
        assert_eq!(forest.superior_of(child_b), None);
        assert!(!forest.subordinates_of(parent).contains(&deceased));
        assert_eq!(forest.validate(), Ok(()));

        forest.succeed(child_a, None).unwrap();
        assert!(!forest.subordinates_of.contains_key(&child_a));
        assert_eq!(forest.validate(), Ok(()));
    }

    #[test]
    fn invalid_successor_and_unknown_unit_leave_forest_unchanged() {
        let (_world, e) = spawn_entities(4);
        let (deceased, child, unrelated, unknown) = (e[0], e[1], e[2], e[3]);
        let mut forest = CommandForest::default();
        forest.set_superior(child, Some(deceased)).unwrap();
        forest.ensure_node(unrelated);
        let original = forest.clone();

        assert_eq!(
            forest.succeed(deceased, Some(unrelated)),
            Err(CommandMutationError::InvalidSuccessor {
                deceased,
                successor: unrelated,
            })
        );
        assert_eq!(forest, original);
        assert_eq!(
            forest.succeed(unknown, None),
            Err(CommandMutationError::UnknownUnit(unknown))
        );
        assert_eq!(forest, original);
    }

    #[test]
    fn validation_detects_cycles_and_denormalized_map_disagreement() {
        let (_world, e) = spawn_entities(3);
        let (a, b, c) = (e[0], e[1], e[2]);
        let cyclic = CommandForest {
            superior_of: [(a, b), (b, a)].into(),
            subordinates_of: [(a, vec![b]), (b, vec![a])].into(),
        };
        assert!(matches!(
            cyclic.validate(),
            Err(CommandForestInvariantError::Cycle(_))
        ));

        let incoherent = CommandForest {
            superior_of: [(c, a)].into(),
            subordinates_of: [(a, Vec::new()), (c, Vec::new())].into(),
        };
        assert_eq!(
            incoherent.validate(),
            Err(CommandForestInvariantError::MissingSubordinateEntry {
                parent: a,
                child: c,
            })
        );

        let duplicate = CommandForest {
            superior_of: [(c, a)].into(),
            subordinates_of: [(a, vec![c, c]), (c, Vec::new())].into(),
        };
        assert_eq!(
            duplicate.validate(),
            Err(CommandForestInvariantError::DuplicateSubordinate {
                parent: a,
                child: c,
            })
        );

        let missing_node = CommandForest {
            superior_of: [(c, a)].into(),
            subordinates_of: [(a, vec![c])].into(),
        };
        assert_eq!(
            missing_node.validate(),
            Err(CommandForestInvariantError::MissingNode(c))
        );
    }

    #[test]
    fn failed_succession_on_invalid_forest_is_atomic() {
        let (_world, e) = spawn_entities(2);
        let (deceased, child) = (e[0], e[1]);
        let mut forest = CommandForest {
            superior_of: [(child, deceased)].into(),
            subordinates_of: [(deceased, Vec::new()), (child, Vec::new())].into(),
        };
        let original = forest.clone();

        assert!(matches!(
            forest.succeed(deceased, Some(child)),
            Err(CommandMutationError::InvalidForest(_))
        ));
        assert_eq!(forest, original);
    }

    #[test]
    fn executable_authority_requires_both_endpoints_to_be_living() {
        let (_world, e) = spawn_entities(3);
        let (leader, subordinate, unrelated) = (e[0], e[1], e[2]);
        let mut forest = CommandForest::default();
        forest.set_superior(subordinate, Some(leader)).unwrap();
        forest.ensure_node(unrelated);

        assert!(forest.can_issue_command(leader, subordinate, |_| true));
        assert!(forest.can_issue_command(leader, leader, |_| true));
        assert!(!forest.can_issue_command(leader, unrelated, |_| true));
        assert!(!forest.can_issue_command(leader, subordinate, |unit| unit != leader));
        assert!(!forest.can_issue_command(leader, subordinate, |unit| unit != subordinate));
        assert!(forest.can_command_in_forest(leader, subordinate));
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
