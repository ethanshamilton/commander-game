use crate::units::{Rank, Role, Side};

pub struct MissionDefinition {
    pub name: &'static str,
    pub units: &'static [MissionUnit],
}

pub struct MissionUnit {
    pub side: Side,
    pub rank: Rank,
    pub role: Role,
    pub position_meters: [f32; 2],
    pub heading_radians: f32,
}

pub const DEMO_MISSION: MissionDefinition = MissionDefinition {
    name: "Demo Mission",
    units: &[
        MissionUnit {
            side: Side::Red,
            rank: Rank::Private,
            role: Role::Rifleman,
            position_meters: [-60.0, 30.0],
            heading_radians: 0.0,
        },
        MissionUnit {
            side: Side::Red,
            rank: Rank::Private,
            role: Role::Rifleman,
            position_meters: [-50.0, 10.0],
            heading_radians: 0.0,
        },
        MissionUnit {
            side: Side::Red,
            rank: Rank::Private,
            role: Role::Rifleman,
            position_meters: [-55.0, -15.0],
            heading_radians: 0.0,
        },
        MissionUnit {
            side: Side::Red,
            rank: Rank::Sergeant,
            role: Role::Rifleman,
            position_meters: [-70.0, -35.0],
            heading_radians: 0.0,
        },
        MissionUnit {
            side: Side::Blue,
            rank: Rank::Private,
            role: Role::Rifleman,
            position_meters: [60.0, 30.0],
            heading_radians: std::f32::consts::PI,
        },
        MissionUnit {
            side: Side::Blue,
            rank: Rank::Private,
            role: Role::Rifleman,
            position_meters: [50.0, 10.0],
            heading_radians: std::f32::consts::PI,
        },
        MissionUnit {
            side: Side::Blue,
            rank: Rank::Private,
            role: Role::Rifleman,
            position_meters: [55.0, -15.0],
            heading_radians: std::f32::consts::PI,
        },
        MissionUnit {
            side: Side::Blue,
            rank: Rank::Sergeant,
            role: Role::Rifleman,
            position_meters: [70.0, -35.0],
            heading_radians: std::f32::consts::PI,
        },
    ],
};
