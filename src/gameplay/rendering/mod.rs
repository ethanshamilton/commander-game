#![doc = include_str!("../../../docs/gameplay/rendering.md")]

mod camera;
mod map;
mod overlays;
mod units;

use crate::gameplay::map::BattlefieldMap;
use bevy::gizmos::config::{DefaultGizmoConfigGroup, GizmoConfigStore};
use bevy::prelude::*;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderingSet {
    Map,
    Units,
    Overlays,
}

pub struct GameplayRenderingPlugin;

impl Plugin for GameplayRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BattlefieldMap>()
            .configure_sets(
                Update,
                (
                    RenderingSet::Map,
                    RenderingSet::Units,
                    RenderingSet::Overlays,
                )
                    .chain(),
            )
            .add_systems(Startup, configure_gizmos)
            .add_plugins((
                camera::BattlefieldCameraPlugin,
                map::MapRenderingPlugin,
                units::UnitRenderingPlugin,
                overlays::TacticalOverlayRenderingPlugin,
            ));
    }
}

fn configure_gizmos(mut config_store: ResMut<GizmoConfigStore>) {
    let (config, _) = config_store.config_mut::<DefaultGizmoConfigGroup>();
    config.line.width = 1.0;
}
