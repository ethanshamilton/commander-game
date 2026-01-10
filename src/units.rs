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

/// Where an entity exists on the map
#[derive(Component)]
pub struct Position {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

// STRUCTS

#[derive(Debug, Clone)]
pub struct Item {
    pub name: String,
}

// ENUMS

#[derive(Debug, Clone, Copy)]
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
