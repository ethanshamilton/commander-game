#![doc = include_str!("../../docs/gameplay/spatial.md")]

use bevy::prelude::*;

/// Horizontal simulation-space location on the tactical battlefield, stored in meters.
///
/// Rendering converts this to Bevy world units at the display boundary. Ground elevation
/// is derived from terrain; future non-ground altitude should extend this spatial model
/// rather than introducing a parallel actor position component.
#[derive(Component, Debug, Clone, Copy)]
pub struct BattlefieldPosition(pub Vec2);

/// Facing direction in radians, where 0 points along +X.
#[derive(Component, Debug, Clone, Copy)]
pub struct Heading(pub f32);
