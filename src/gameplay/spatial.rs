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

/// A requested battlefield pose. `None` preserves the unit's arrival heading;
/// `Some` requires it to face that direction after reaching the position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionTarget {
    pub position_m: Vec2,
    pub heading_radians: Option<f32>,
}

impl PositionTarget {
    pub const fn new(position_m: Vec2, heading_radians: Option<f32>) -> Self {
        Self {
            position_m,
            heading_radians,
        }
    }

    pub fn is_reached(
        self,
        position_m: Vec2,
        heading_radians: f32,
        position_epsilon_m: f32,
        heading_epsilon_radians: f32,
    ) -> bool {
        position_m.distance(self.position_m) <= position_epsilon_m
            && self.heading_radians.is_none_or(|target_heading| {
                angular_distance(heading_radians, target_heading) <= heading_epsilon_radians
            })
    }
}

pub fn angular_distance(a: f32, b: f32) -> f32 {
    let difference = a - b;
    difference.sin().atan2(difference.cos()).abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_target_checks_position_and_optional_heading() {
        let target = PositionTarget::new(Vec2::X, Some(std::f32::consts::PI));
        assert!(target.is_reached(Vec2::new(1.01, 0.0), -std::f32::consts::PI, 0.02, 0.01));
        assert!(!target.is_reached(Vec2::X, 0.0, 0.02, 0.01));

        let position_only = PositionTarget::new(Vec2::X, None);
        assert!(position_only.is_reached(Vec2::X, 0.0, 0.02, 0.01));
    }
}
