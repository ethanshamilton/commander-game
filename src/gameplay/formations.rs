#![doc = include_str!("../../docs/gameplay/formations.md")]

use bevy::prelude::*;

/// Shape used to generate coordinator-relative formation positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormationKind {
    Wedge,
}

/// Geometry of a formation. Position zero is always the coordinator at the
/// anchor; all other positions are generated behind it relative to `facing_radians`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormationSpec {
    pub kind: FormationKind,
    pub anchor_m: Vec2,
    pub facing_radians: f32,
    pub lateral_spacing_m: f32,
    pub depth_spacing_m: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormationValidationError {
    NonFiniteAnchor,
    NonFiniteFacing,
    InvalidSpacing,
}

/// Generate exactly `count` absolute positions with no fixed formation-size
/// limit. The returned order is stable and position zero is the coordinator.
pub fn generate_formation_positions(
    spec: FormationSpec,
    count: usize,
) -> Result<Vec<Vec2>, FormationValidationError> {
    validate_spec(spec)?;
    match spec.kind {
        FormationKind::Wedge => Ok(generate_wedge_positions(spec, count)),
    }
}

fn validate_spec(spec: FormationSpec) -> Result<(), FormationValidationError> {
    if !spec.anchor_m.is_finite() {
        return Err(FormationValidationError::NonFiniteAnchor);
    }
    if !spec.facing_radians.is_finite() {
        return Err(FormationValidationError::NonFiniteFacing);
    }
    if !spec.lateral_spacing_m.is_finite()
        || !spec.depth_spacing_m.is_finite()
        || spec.lateral_spacing_m <= 0.0
        || spec.depth_spacing_m <= 0.0
    {
        return Err(FormationValidationError::InvalidSpacing);
    }
    Ok(())
}

fn generate_wedge_positions(spec: FormationSpec, count: usize) -> Vec<Vec2> {
    if count == 0 {
        return Vec::new();
    }

    let forward = Vec2::from_angle(spec.facing_radians);
    let right = forward.perp();
    let mut positions = Vec::with_capacity(count);
    positions.push(spec.anchor_m);

    let mut row = 1usize;
    while positions.len() < count {
        let row_capacity = row * 2;
        for column in 0..row_capacity {
            if positions.len() == count {
                break;
            }
            let lateral_units = column as f32 - (row_capacity - 1) as f32 / 2.0;
            let offset = right * lateral_units * spec.lateral_spacing_m
                - forward * row as f32 * spec.depth_spacing_m;
            positions.push(spec.anchor_m + offset);
        }
        row += 1;
    }

    positions
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn wedge() -> FormationSpec {
        FormationSpec {
            kind: FormationKind::Wedge,
            anchor_m: Vec2::new(10.0, 20.0),
            facing_radians: 0.0,
            lateral_spacing_m: 2.0,
            depth_spacing_m: 3.0,
        }
    }

    #[test]
    fn wedge_generates_exactly_the_requested_number_of_unique_positions() {
        for count in [0, 1, 2, 3, 10, 100, 10_000] {
            let positions = generate_formation_positions(wedge(), count).unwrap();
            assert_eq!(positions.len(), count);
            assert!(positions.iter().all(|position| position.is_finite()));
            let quantized: HashSet<_> = positions
                .iter()
                .map(|position| {
                    (
                        (position.x * 100.0).round() as i32,
                        (position.y * 100.0).round() as i32,
                    )
                })
                .collect();
            assert_eq!(quantized.len(), count);
        }
    }

    #[test]
    fn wedge_places_coordinator_at_anchor_and_rows_behind_facing() {
        let positions = generate_formation_positions(wedge(), 7).unwrap();
        assert_eq!(positions[0], Vec2::new(10.0, 20.0));
        assert_eq!(positions[1], Vec2::new(7.0, 19.0));
        assert_eq!(positions[2], Vec2::new(7.0, 21.0));
        assert!(positions[3..].iter().all(|position| position.x == 4.0));
    }

    #[test]
    fn formation_rotation_changes_world_orientation() {
        let mut spec = wedge();
        spec.anchor_m = Vec2::ZERO;
        spec.facing_radians = std::f32::consts::FRAC_PI_2;
        let positions = generate_formation_positions(spec, 3).unwrap();

        assert!(positions[1].y < 0.0);
        assert!(positions[2].y < 0.0);
        assert!(positions[1].x.signum() != positions[2].x.signum());
    }

    #[test]
    fn invalid_geometry_is_rejected() {
        let mut spec = wedge();
        spec.lateral_spacing_m = 0.0;
        assert_eq!(
            generate_formation_positions(spec, 4),
            Err(FormationValidationError::InvalidSpacing)
        );

        spec = wedge();
        spec.anchor_m.x = f32::NAN;
        assert_eq!(
            generate_formation_positions(spec, 4),
            Err(FormationValidationError::NonFiniteAnchor)
        );
    }
}
