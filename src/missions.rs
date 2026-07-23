#![doc = include_str!("../docs/missions.md")]

use crate::actors::units::{Rank, Role, Side};
use crate::gameplay::command::{CommandAssignmentDefinition, UnitId};
use crate::gameplay::objectives::MissionCondition;
use crate::gameplay::squads::{SquadDefinition, SquadId};
use crate::maps::{DEMO_MAP, MapDefinition};
use bevy::prelude::Resource;

pub struct MissionDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub briefing: &'static str,
    pub map: &'static MapDefinition,
    pub units: &'static [MissionUnit],
    pub squads: &'static [SquadDefinition],
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

#[derive(Resource, Clone, Copy)]
pub struct SelectedMission {
    pub mission: &'static MissionDefinition,
}

pub const TUTORIAL_MISSIONS: &[&MissionDefinition] = &[&SINGLE_SQUAD_COMMAND_TUTORIAL_SCENARIO];

pub const SINGLE_SQUAD_COMMAND_TUTORIAL_SCENARIO: MissionDefinition = MissionDefinition {
    id: "single_squad_command",
    name: "Tutorial: Single Squad Command",
    briefing: "Command a single squad, maintain contact with your soldiers, and eliminate all hostile units.",
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
    squads: &[
        SquadDefinition {
            id: SquadId("blue_squad"),
            label: "Blue Squad",
            members: &[
                UnitId("blue_sergeant"),
                UnitId("blue_rifleman_1"),
                UnitId("blue_rifleman_2"),
                UnitId("blue_rifleman_3"),
            ],
        },
        SquadDefinition {
            id: SquadId("red_squad"),
            label: "Red Squad",
            members: &[
                UnitId("red_sergeant"),
                UnitId("red_rifleman_1"),
                UnitId("red_rifleman_2"),
                UnitId("red_rifleman_3"),
            ],
        },
    ],
    command_assignments: &[],
    victory_conditions: &[MissionCondition::AllHostilesEliminated],
    defeat_conditions: &[MissionCondition::AllFriendliesEliminated],
};
