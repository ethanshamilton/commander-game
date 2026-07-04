#![doc = include_str!("../../../docs/ai/htn.md")]
pub mod domain;
pub mod executor;
pub mod planner;
pub mod soldier;
pub mod state;
pub mod synthesis;
pub mod trace;

use bevy::prelude::*;
use executor::{HtnDomainRegistry, HtnExecutorPlugin};

pub struct HtnPlugin;

impl Plugin for HtnPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HtnExecutorPlugin)
            .insert_resource(HtnDomainRegistry {
                soldier: Some(soldier::build_soldier_domain()),
            });
    }
}
