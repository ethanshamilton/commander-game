use crate::gameplay::components::{BattlefieldPosition, Heading};
use crate::units::{Allegiance, Side, Soldier};
use crate::GameState;
use bevy::prelude::*;

pub struct GameplayRenderingPlugin;

impl Plugin for GameplayRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BattlefieldMap>().add_systems(
            Update,
            (draw_battlefield_grid, draw_units).run_if(in_state(GameState::MissionScreen)),
        );
    }
}

#[derive(Resource, Debug, Clone)]
pub struct BattlefieldMap {
    pub cells: UVec2,
    pub cell_size: f32,
}

impl Default for BattlefieldMap {
    fn default() -> Self {
        Self {
            cells: UVec2::new(32, 24),
            cell_size: 32.0,
        }
    }
}

fn draw_battlefield_grid(mut gizmos: Gizmos, map: Res<BattlefieldMap>) {
    let grid_color = Color::srgba(0.0, 0.8, 0.25, 0.18);
    let border_color = Color::srgba(0.0, 1.0, 0.35, 0.55);
    let axis_color = Color::srgba(0.0, 1.0, 0.35, 0.35);

    gizmos
        .grid_2d(
            Isometry2d::IDENTITY,
            map.cells,
            Vec2::splat(map.cell_size),
            grid_color,
        )
        .outer_edges();

    let size = map.cells.as_vec2() * map.cell_size;
    gizmos.rect_2d(Isometry2d::IDENTITY, size, border_color);

    // Center axes give the empty map a radar-screen feel and establish orientation.
    gizmos.line_2d(Vec2::new(-size.x / 2.0, 0.0), Vec2::new(size.x / 2.0, 0.0), axis_color);
    gizmos.line_2d(Vec2::new(0.0, -size.y / 2.0), Vec2::new(0.0, size.y / 2.0), axis_color);

    // Range rings.
    for radius in [128.0, 256.0, 384.0] {
        gizmos.circle_2d(Isometry2d::IDENTITY, radius, axis_color).resolution(96);
    }
}

fn draw_units(
    units: Query<(&BattlefieldPosition, Option<&Heading>, &Allegiance), With<Soldier>>,
    mut gizmos: Gizmos,
) {
    for (position, heading, allegiance) in &units {
        let color = match allegiance.side {
            Side::Blue => Color::srgb(0.0, 0.85, 1.0),
            Side::Red => Color::srgb(1.0, 0.15, 0.1),
        };

        let p = position.0;
        gizmos.circle_2d(p, 7.0, color).resolution(24);
        gizmos.cross_2d(p, 10.0, color);

        if let Some(Heading(angle)) = heading {
            let facing = Vec2::from_angle(*angle) * 20.0;
            gizmos.line_2d(p, p + facing, color);
        }
    }
}
