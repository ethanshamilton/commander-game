#![doc = include_str!("../../docs/gameplay/missions.md")]

use crate::GameState;
use bevy::prelude::*;

/// Tactical missions are persistent intent-bearing plans created during a
/// scenario. They are deliberately distinct from authored `ScenarioDefinition`
/// data and never install concrete action orders directly.
pub struct MissionsPlugin;

impl Plugin for MissionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MissionIdAllocator>().add_systems(
            OnEnter(GameState::ScenarioScreen),
            reset_mission_id_allocator,
        );
    }
}

/// Stable, scenario-local identifier for a tactical mission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MissionId(pub u64);

/// Allocates monotonically increasing tactical mission IDs within one scenario.
#[derive(Resource, Debug, Default)]
pub struct MissionIdAllocator {
    next: u64,
}

impl MissionIdAllocator {
    pub fn allocate(&mut self) -> MissionId {
        let id = MissionId(self.next);
        self.next = self
            .next
            .checked_add(1)
            .expect("tactical mission ID allocator exhausted");
        id
    }

    pub fn reset(&mut self) {
        self.next = 0;
    }
}

fn reset_mission_id_allocator(mut allocator: ResMut<MissionIdAllocator>) {
    allocator.reset();
}

/// Geometry defining where a tactical mission is to be executed. All positions
/// are in meters, not Bevy render units.
#[derive(Debug, Clone, PartialEq)]
pub enum MissionArea {
    Line { from_m: Vec2, to_m: Vec2 },
    Point { center_m: Vec2 },
    Circle { center_m: Vec2, radius_m: f32 },
    Rect { min_m: Vec2, max_m: Vec2 },
}

impl MissionArea {
    pub fn shape_name(&self) -> &'static str {
        match self {
            Self::Line { .. } => "line",
            Self::Point { .. } => "point",
            Self::Circle { .. } => "circle",
            Self::Rect { .. } => "rectangle",
        }
    }

    fn validate_geometry(&self) -> Result<(), MissionValidationError> {
        match self {
            Self::Line { from_m, to_m } => {
                validate_finite_point("line start", *from_m)?;
                validate_finite_point("line end", *to_m)?;
                if from_m.distance_squared(*to_m) <= f32::EPSILON {
                    return Err(MissionValidationError::DegenerateLine);
                }
            }
            Self::Point { center_m } => validate_finite_point("point center", *center_m)?,
            Self::Circle { center_m, radius_m } => {
                validate_finite_point("circle center", *center_m)?;
                if !radius_m.is_finite() {
                    return Err(MissionValidationError::NonFiniteRadius);
                }
                if *radius_m <= 0.0 {
                    return Err(MissionValidationError::NonPositiveRadius);
                }
            }
            Self::Rect { min_m, max_m } => {
                validate_finite_point("rectangle minimum", *min_m)?;
                validate_finite_point("rectangle maximum", *max_m)?;
                if min_m.x >= max_m.x || min_m.y >= max_m.y {
                    return Err(MissionValidationError::DegenerateOrUnnormalizedRect);
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MissionKind {
    HoldLine,
    SecurePerimeter,
    ScoutArea,
    ClearArea,
}

impl MissionKind {
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::HoldLine => "Hold Line",
            Self::SecurePerimeter => "Secure Perimeter",
            Self::ScoutArea => "Scout Area",
            Self::ClearArea => "Clear Area",
        }
    }

    pub fn accepts_area(self, area: &MissionArea) -> bool {
        matches!(
            (self, area),
            (Self::HoldLine, MissionArea::Line { .. })
                | (
                    Self::SecurePerimeter,
                    MissionArea::Circle { .. } | MissionArea::Rect { .. }
                )
                | (
                    Self::ScoutArea,
                    MissionArea::Point { .. }
                        | MissionArea::Circle { .. }
                        | MissionArea::Rect { .. }
                )
                | (
                    Self::ClearArea,
                    MissionArea::Point { .. }
                        | MissionArea::Circle { .. }
                        | MissionArea::Rect { .. }
                )
        )
    }
}

/// Persistent player-authored tactical mission plan.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct MissionPlan {
    pub id: MissionId,
    pub label: String,
    pub kind: MissionKind,
    pub area: MissionArea,
    pub rally_point_m: Vec2,
    pub expires_at: Option<u64>,
    pub created_tick: u64,
}

impl MissionPlan {
    pub fn validate(&self) -> Result<(), MissionValidationError> {
        validate_mission_fields(
            &self.label,
            self.kind,
            &self.area,
            self.rally_point_m,
            self.expires_at,
            self.created_tick,
        )
    }

    pub fn snapshot(&self) -> MissionSnapshot {
        MissionSnapshot {
            id: self.id,
            label: self.label.clone(),
            kind: self.kind,
            area: self.area.clone(),
            rally_point_m: self.rally_point_m,
            expires_at: self.expires_at,
            created_tick: self.created_tick,
        }
    }
}

/// Marks an entity as a tactical mission world object rather than an authored
/// scenario entity or a unit. It should be paired with `ScenarioScoped` when
/// missions are spawned so scenario cleanup despawns it.
#[allow(dead_code)] // Constructed by milestone C mission placement.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct TacticalMission;

/// Persistent local record of which units the player assigned to a mission.
#[allow(dead_code)] // Constructed by milestone C and mutated by milestone D.
#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
pub struct MissionAssignees {
    pub assignees: Vec<Entity>,
}

/// Copy of a mission plan that is safe to transmit through the comms substrate.
/// It contains no local Bevy entity reference.
#[derive(Debug, Clone, PartialEq)]
pub struct MissionSnapshot {
    pub id: MissionId,
    pub label: String,
    pub kind: MissionKind,
    pub area: MissionArea,
    pub rally_point_m: Vec2,
    pub expires_at: Option<u64>,
    pub created_tick: u64,
}

impl MissionSnapshot {
    pub fn validate(&self) -> Result<(), MissionValidationError> {
        validate_mission_fields(
            &self.label,
            self.kind,
            &self.area,
            self.rally_point_m,
            self.expires_at,
            self.created_tick,
        )
    }
}

/// Mission intent installed on the receiving leader after a valid assignment
/// packet arrives. This is an HTN input, never a concrete action order.
#[allow(dead_code)] // Installed by milestone D packet handling.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct AssignedMission {
    pub mission: MissionSnapshot,
    pub assigned_by: Entity,
    pub issued_tick: u64,
    pub received_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissionValidationError {
    EmptyLabel,
    IncompatibleArea {
        kind: MissionKind,
        shape: &'static str,
    },
    NonFinitePoint {
        field: &'static str,
    },
    NonFiniteRadius,
    NonPositiveRadius,
    DegenerateLine,
    DegenerateOrUnnormalizedRect,
    ExpiryNotAfterCreation {
        expires_at: u64,
        created_tick: u64,
    },
}

fn validate_mission_fields(
    label: &str,
    kind: MissionKind,
    area: &MissionArea,
    rally_point_m: Vec2,
    expires_at: Option<u64>,
    created_tick: u64,
) -> Result<(), MissionValidationError> {
    if label.trim().is_empty() {
        return Err(MissionValidationError::EmptyLabel);
    }
    if !kind.accepts_area(area) {
        return Err(MissionValidationError::IncompatibleArea {
            kind,
            shape: area.shape_name(),
        });
    }

    area.validate_geometry()?;
    validate_finite_point("rally point", rally_point_m)?;

    if let Some(expires_at) = expires_at
        && expires_at <= created_tick
    {
        return Err(MissionValidationError::ExpiryNotAfterCreation {
            expires_at,
            created_tick,
        });
    }

    Ok(())
}

fn validate_finite_point(field: &'static str, point: Vec2) -> Result<(), MissionValidationError> {
    if point.is_finite() {
        Ok(())
    } else {
        Err(MissionValidationError::NonFinitePoint { field })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hold_line() -> MissionPlan {
        MissionPlan {
            id: MissionId(4),
            label: "Alpha line".into(),
            kind: MissionKind::HoldLine,
            area: MissionArea::Line {
                from_m: Vec2::new(-10.0, 0.0),
                to_m: Vec2::new(10.0, 0.0),
            },
            rally_point_m: Vec2::new(0.0, -5.0),
            expires_at: Some(101),
            created_tick: 100,
        }
    }

    #[test]
    fn valid_hold_line_plan_and_snapshot_validate() {
        let plan = hold_line();
        assert_eq!(plan.validate(), Ok(()));
        assert_eq!(plan.snapshot().validate(), Ok(()));
    }

    #[test]
    fn kinds_accept_only_supported_area_shapes() {
        let line = MissionArea::Line {
            from_m: Vec2::ZERO,
            to_m: Vec2::X,
        };
        let point = MissionArea::Point {
            center_m: Vec2::ZERO,
        };
        let circle = MissionArea::Circle {
            center_m: Vec2::ZERO,
            radius_m: 1.0,
        };
        let rect = MissionArea::Rect {
            min_m: Vec2::ZERO,
            max_m: Vec2::ONE,
        };

        assert!(MissionKind::HoldLine.accepts_area(&line));
        assert!(!MissionKind::HoldLine.accepts_area(&point));
        assert!(MissionKind::SecurePerimeter.accepts_area(&circle));
        assert!(MissionKind::SecurePerimeter.accepts_area(&rect));
        assert!(!MissionKind::SecurePerimeter.accepts_area(&point));
        assert!(MissionKind::ScoutArea.accepts_area(&point));
        assert!(MissionKind::ClearArea.accepts_area(&rect));
    }

    #[test]
    fn validation_rejects_invalid_geometry_and_metadata() {
        let mut plan = hold_line();
        plan.area = MissionArea::Line {
            from_m: Vec2::ZERO,
            to_m: Vec2::ZERO,
        };
        assert_eq!(plan.validate(), Err(MissionValidationError::DegenerateLine));

        plan = hold_line();
        plan.area = MissionArea::Circle {
            center_m: Vec2::ZERO,
            radius_m: 0.0,
        };
        plan.kind = MissionKind::SecurePerimeter;
        assert_eq!(
            plan.validate(),
            Err(MissionValidationError::NonPositiveRadius)
        );

        plan = hold_line();
        plan.area = MissionArea::Rect {
            min_m: Vec2::new(2.0, 0.0),
            max_m: Vec2::new(1.0, 1.0),
        };
        plan.kind = MissionKind::ScoutArea;
        assert_eq!(
            plan.validate(),
            Err(MissionValidationError::DegenerateOrUnnormalizedRect)
        );

        plan = hold_line();
        plan.rally_point_m = Vec2::new(f32::NAN, 0.0);
        assert_eq!(
            plan.validate(),
            Err(MissionValidationError::NonFinitePoint {
                field: "rally point"
            })
        );

        plan = hold_line();
        plan.expires_at = Some(plan.created_tick);
        assert_eq!(
            plan.validate(),
            Err(MissionValidationError::ExpiryNotAfterCreation {
                expires_at: 100,
                created_tick: 100,
            })
        );
    }

    #[test]
    fn validation_rejects_empty_labels_and_incompatible_areas() {
        let mut plan = hold_line();
        plan.label = " \t".into();
        assert_eq!(plan.validate(), Err(MissionValidationError::EmptyLabel));

        plan = hold_line();
        plan.area = MissionArea::Point {
            center_m: Vec2::ZERO,
        };
        assert_eq!(
            plan.validate(),
            Err(MissionValidationError::IncompatibleArea {
                kind: MissionKind::HoldLine,
                shape: "point",
            })
        );
    }

    #[test]
    fn allocator_is_monotonic_and_resettable() {
        let mut allocator = MissionIdAllocator::default();
        assert_eq!(allocator.allocate(), MissionId(0));
        assert_eq!(allocator.allocate(), MissionId(1));
        allocator.reset();
        assert_eq!(allocator.allocate(), MissionId(0));
    }

    #[test]
    fn entering_a_scenario_resets_the_mission_id_allocator() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<GameState>()
            .add_plugins(MissionsPlugin);

        {
            let mut allocator = app.world_mut().resource_mut::<MissionIdAllocator>();
            assert_eq!(allocator.allocate(), MissionId(0));
            assert_eq!(allocator.allocate(), MissionId(1));
        }

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::ScenarioScreen);
        app.update();

        assert_eq!(
            app.world_mut()
                .resource_mut::<MissionIdAllocator>()
                .allocate(),
            MissionId(0)
        );
    }
}
