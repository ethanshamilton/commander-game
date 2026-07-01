use bevy::prelude::*;

#[derive(Component, Debug, Clone, Copy)]
pub struct Weapon {
    pub max_range_m: f32,
    pub effective_range_m: f32,
    pub damage: i32,
    pub base_accuracy: f32,
    pub cooldown_ticks: u64,
    pub projectile_speed_mps: f32,
    pub tracer_length_m: f32,
}

impl Weapon {
    pub fn default_rifle() -> Self {
        Self {
            max_range_m: 140.0,
            effective_range_m: 70.0,
            damage: 35,
            base_accuracy: 0.55,
            cooldown_ticks: 20,
            projectile_speed_mps: 700.0,
            tracer_length_m: 8.0,
        }
    }
}
