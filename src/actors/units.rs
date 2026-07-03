#![doc = include_str!("../../docs/units.md")]
#![allow(dead_code)] // allow temporarily while sketching
use bevy::prelude::*;

// BUNDLES

#[derive(Bundle)]
pub struct SoldierBundle {
    pub soldier: Soldier,
    pub allegiance: Allegiance,
    pub health: Health,
    pub mobility: Mobility,
    pub inventory: Inventory,
}

// COMPONENTS

#[derive(Component)]
pub struct Alive;

#[derive(Component)]
pub struct Dead;

#[derive(Component)]
pub struct Allegiance {
    pub side: Side,
}

// Individual soldier metadata
#[derive(Component)]
pub struct Soldier {
    pub rank: Rank,
    pub role: Role,
}

#[derive(Component)]
pub struct Health {
    pub current: i32,
    pub max: i32,
}

/// Movement stats for entity
#[derive(Component)]
pub struct Mobility {
    pub speed: i32,
}

#[derive(Component)]
pub struct Inventory {
    pub items: Vec<Item>,
}

impl Inventory {
    pub fn ammo_count(&self) -> u32 {
        self.items
            .iter()
            .filter(|item| item.kind == ItemKind::Ammo)
            .map(|item| item.count)
            .sum()
    }

    pub fn has_ammo(&self) -> bool {
        self.ammo_count() > 0
    }

    pub fn consume_ammo(&mut self, amount: u32) -> bool {
        if amount == 0 {
            return true;
        }

        if self.ammo_count() < amount {
            return false;
        }

        let mut remaining = amount;
        for item in self
            .items
            .iter_mut()
            .filter(|item| item.kind == ItemKind::Ammo)
        {
            let spent = item.count.min(remaining);
            item.count -= spent;
            remaining -= spent;

            if remaining == 0 {
                break;
            }
        }

        true
    }
}

// STRUCTS

#[derive(Debug, Clone)]
pub struct Item {
    pub kind: ItemKind,
    pub count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Ammo,
}

// ENUMS

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Blue,
    Red,
}

#[derive(Debug, Clone, Copy)]
pub enum Rank {
    Private,
    Sergeant,
    Lieutenant,
    Colonel,
    Commander,
}

#[derive(Debug, Clone, Copy)]
pub enum Role {
    Rifleman,
    Communications,
    Medic,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_reports_and_consumes_ammo() {
        let mut inventory = Inventory {
            items: vec![Item {
                kind: ItemKind::Ammo,
                count: 3,
            }],
        };

        assert_eq!(inventory.ammo_count(), 3);
        assert!(inventory.has_ammo());
        assert!(inventory.consume_ammo(1));
        assert_eq!(inventory.ammo_count(), 2);
        assert!(inventory.consume_ammo(2));
        assert_eq!(inventory.ammo_count(), 0);
        assert!(!inventory.has_ammo());
        assert!(!inventory.consume_ammo(1));
    }

    #[test]
    fn consuming_zero_ammo_is_noop_success() {
        let mut inventory = Inventory { items: vec![] };

        assert!(inventory.consume_ammo(0));
        assert_eq!(inventory.ammo_count(), 0);
    }

    #[test]
    fn ammo_consumption_can_span_stacks() {
        let mut inventory = Inventory {
            items: vec![
                Item {
                    kind: ItemKind::Ammo,
                    count: 1,
                },
                Item {
                    kind: ItemKind::Ammo,
                    count: 2,
                },
            ],
        };

        assert!(inventory.consume_ammo(3));
        assert_eq!(inventory.ammo_count(), 0);
    }
}
