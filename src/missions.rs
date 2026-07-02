#![doc = include_str!("../docs/missions.md")]

use crate::actors::units::{Rank, Role, Side};
use crate::gameplay::command::{CommandAssignmentDefinition, UnitId};
use crate::gameplay::objectives::MissionCondition;
use crate::maps::{DEMO_MAP, MapDefinition};

pub struct MissionDefinition {
    pub name: &'static str,
    pub map: &'static MapDefinition,
    pub units: &'static [MissionUnit],
    pub command_assignments: &'static [CommandAssignmentDefinition],
    pub victory_conditions: &'static [MissionCondition],
    pub defeat_conditions: &'static [MissionCondition],
}

pub struct MissionUnit {
    pub id: UnitId,
    pub side: Side,
    pub rank: Rank,
    pub role: Role,
    pub position_meters: [f32; 2],
    pub heading_radians: f32,
}

pub const DEMO_MISSION: MissionDefinition = MissionDefinition {
    name: "Demo Mission",
    map: &DEMO_MAP,
    units: &[
        MissionUnit {
            id: UnitId("red_rifleman_1"),
            side: Side::Red,
            rank: Rank::Private,
            role: Role::Rifleman,
            position_meters: [-60.0, 30.0],
            heading_radians: 0.0,
        },
        MissionUnit {
            id: UnitId("red_rifleman_2"),
            side: Side::Red,
            rank: Rank::Private,
            role: Role::Rifleman,
            position_meters: [-50.0, 10.0],
            heading_radians: 0.0,
        },
        MissionUnit {
            id: UnitId("red_rifleman_3"),
            side: Side::Red,
            rank: Rank::Private,
            role: Role::Rifleman,
            position_meters: [-55.0, -15.0],
            heading_radians: 0.0,
        },
        MissionUnit {
            id: UnitId("red_sergeant"),
            side: Side::Red,
            rank: Rank::Sergeant,
            role: Role::Rifleman,
            position_meters: [-70.0, -35.0],
            heading_radians: 0.0,
        },
        MissionUnit {
            id: UnitId("blue_rifleman_1"),
            side: Side::Blue,
            rank: Rank::Private,
            role: Role::Rifleman,
            position_meters: [60.0, 30.0],
            heading_radians: std::f32::consts::PI,
        },
        MissionUnit {
            id: UnitId("blue_rifleman_2"),
            side: Side::Blue,
            rank: Rank::Private,
            role: Role::Rifleman,
            position_meters: [50.0, 10.0],
            heading_radians: std::f32::consts::PI,
        },
        MissionUnit {
            id: UnitId("blue_rifleman_3"),
            side: Side::Blue,
            rank: Rank::Private,
            role: Role::Rifleman,
            position_meters: [55.0, -15.0],
            heading_radians: std::f32::consts::PI,
        },
        MissionUnit {
            id: UnitId("blue_sergeant"),
            side: Side::Blue,
            rank: Rank::Sergeant,
            role: Role::Rifleman,
            position_meters: [70.0, -35.0],
            heading_radians: std::f32::consts::PI,
        },
    ],
    command_assignments: &[
        CommandAssignmentDefinition {
            subordinate: UnitId("blue_sergeant"),
            superior: None,
        },
        CommandAssignmentDefinition {
            subordinate: UnitId("blue_rifleman_1"),
            superior: Some(UnitId("blue_sergeant")),
        },
        CommandAssignmentDefinition {
            subordinate: UnitId("blue_rifleman_2"),
            superior: Some(UnitId("blue_sergeant")),
        },
        CommandAssignmentDefinition {
            subordinate: UnitId("blue_rifleman_3"),
            superior: Some(UnitId("blue_sergeant")),
        },
        CommandAssignmentDefinition {
            subordinate: UnitId("red_sergeant"),
            superior: None,
        },
        CommandAssignmentDefinition {
            subordinate: UnitId("red_rifleman_1"),
            superior: Some(UnitId("red_sergeant")),
        },
        CommandAssignmentDefinition {
            subordinate: UnitId("red_rifleman_2"),
            superior: Some(UnitId("red_sergeant")),
        },
        CommandAssignmentDefinition {
            subordinate: UnitId("red_rifleman_3"),
            superior: Some(UnitId("red_sergeant")),
        },
    ],
    victory_conditions: &[MissionCondition::AllHostilesEliminated],
    defeat_conditions: &[MissionCondition::AllFriendliesEliminated],
};
