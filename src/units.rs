#![allow(dead_code)] // allow temporarily while sketching
use bevy::prelude::*;

// COMPONENTS

#[derive(Component)]
pub struct Allegiance {
    pub side: Side,
}

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

#[derive(Component)]
pub struct Fitness {
    pub speed: i32,
}

#[derive(Component)]
pub struct Inventory {
    pub items: Vec<Item>,
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
