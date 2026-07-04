#![doc = include_str!("../../../docs/ai/htn.md")]
#![allow(dead_code)]

pub mod domain;
pub mod executor;
pub mod planner;
pub mod state;
pub mod synthesis;
pub mod trace;

use bevy::prelude::*;
use executor::HtnExecutorPlugin;

pub struct HtnPlugin;

impl Plugin for HtnPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HtnExecutorPlugin);
    }
}
