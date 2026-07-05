#![doc = include_str!("../../../docs/ai/htn.md")]
pub mod domain;
pub mod executor;
pub mod operators;
pub mod planner;
pub mod soldier;
pub mod state;
pub mod synthesis;
pub mod trace;

use bevy::prelude::*;
use executor::{DomainId, HtnDomainRegistry, HtnExecutorPlugin};
use std::collections::HashMap;

pub struct HtnPlugin;

impl Plugin for HtnPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HtnExecutorPlugin)
            .insert_resource(HtnDomainRegistry {
                domains: HashMap::from([(DomainId::Soldier, soldier::build_soldier_domain())]),
            });
    }
}
