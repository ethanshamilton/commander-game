use bevy::prelude::*;

/// Simulation-space location on the tactical battlefield, stored in meters.
///
/// Rendering converts this to Bevy world units at the display boundary.
#[derive(Component, Debug, Clone, Copy)]
pub struct BattlefieldPosition(pub Vec2);

/// Facing direction in radians, where 0 points along +X.
#[derive(Component, Debug, Clone, Copy)]
pub struct Heading(pub f32);
