use bevy::prelude::*;

/// World-space location on the tactical battlefield/radar display.
///
/// Coordinates are stored in Bevy world units. Gameplay convention:
/// 10 Bevy units = 1 meter.
#[derive(Component, Debug, Clone, Copy)]
pub struct BattlefieldPosition(pub Vec2);

/// Facing direction in radians, where 0 points along +X.
#[derive(Component, Debug, Clone, Copy)]
pub struct Heading(pub f32);
