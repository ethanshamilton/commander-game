use bevy::prelude::*;

#[derive(Component, Debug, Clone, Copy)]
pub struct Marksmanship {
    pub value: f32,
}

impl Default for Marksmanship {
    fn default() -> Self {
        Self { value: 1.0 }
    }
}
