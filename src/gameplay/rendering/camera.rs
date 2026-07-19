#![doc = include_str!("../../../docs/gameplay/rendering/camera.md")]

use crate::GameState;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;

pub struct BattlefieldCameraPlugin;

impl Plugin for BattlefieldCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (pan_battlefield_camera, zoom_battlefield_camera)
                .run_if(in_state(GameState::MissionScreen)),
        );
    }
}

const MIN_CAMERA_SCALE: f32 = 0.25;
const MAX_CAMERA_SCALE: f32 = 4.0;
const ZOOM_SENSITIVITY: f32 = 0.2;

fn pan_battlefield_camera(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut cameras: Query<(&mut Transform, &Projection), With<Camera2d>>,
) {
    if !mouse_buttons.pressed(MouseButton::Middle) || mouse_motion.delta == Vec2::ZERO {
        return;
    }

    for (mut transform, projection) in &mut cameras {
        let Projection::Orthographic(orthographic) = projection else {
            continue;
        };

        let pan_delta = mouse_motion.delta * orthographic.scale;
        transform.translation.x -= pan_delta.x;
        transform.translation.y += pan_delta.y;
    }
}

fn zoom_battlefield_camera(
    scroll: Res<AccumulatedMouseScroll>,
    mut cameras: Query<&mut Projection, With<Camera2d>>,
) {
    if scroll.delta.y == 0.0 {
        return;
    }

    let scroll_lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR,
    };

    let zoom_factor = (-scroll_lines * ZOOM_SENSITIVITY).exp();

    for mut projection in &mut cameras {
        if let Projection::Orthographic(orthographic) = projection.as_mut() {
            orthographic.scale =
                (orthographic.scale * zoom_factor).clamp(MIN_CAMERA_SCALE, MAX_CAMERA_SCALE);
        }
    }
}
