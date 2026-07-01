use bevy::prelude::*;

#[allow(dead_code)]
#[derive(Message, Debug, Clone, Copy)]
pub struct ResolvedShot {
    pub shooter: Entity,
    pub target: Entity,
    pub shooter_position_m: Vec2,
    pub target_position_m: Vec2,
    pub impact_position_m: Vec2,
    pub hit: bool,
    pub damage: i32,
    pub projectile_speed_mps: f32,
    pub tracer_length_m: f32,
}
