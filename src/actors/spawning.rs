use crate::actors::skills::Marksmanship;
use crate::actors::units::{
    Alive, Allegiance, Health, Inventory, Item, ItemKind, Mobility, Rank, Role, Side, Soldier,
};
use crate::actors::weapons::Weapon;
use crate::ai::htn::executor::{Autonomous, DomainId, DomainRef};
use crate::ai::htn::synthesis::PlannerBelief;
use crate::ai::htn::trace::DecisionTrace;
use crate::ai::perception::{
    AuditorySensor, EyeHeight, PerceptionMemory, SensorSignature, VisualSensor,
};
use crate::gameplay::combat::{CombatOrder, CombatState};
use crate::gameplay::command_plans::CommandPlanDelegationProgress;
use crate::gameplay::comms::{CommsLinks, VoiceComms};
use crate::gameplay::lifecycle::MissionScoped;
use crate::gameplay::orders::CombatOrderSource;
use crate::gameplay::packets::{Inbox, Outbox, SeenPackets};
use crate::gameplay::spatial::{BattlefieldPosition, Heading};
use crate::player::knowledge::ReportCadence;
use bevy::prelude::*;

/// Complete authoring input needed to create a valid soldier entity.
#[derive(Debug, Clone, Copy)]
pub struct SoldierSpawn {
    pub rank: Rank,
    pub role: Role,
    pub side: Side,
    pub position_m: Vec2,
    pub heading_radians: f32,
}

/// The authoritative soldier constructor used by mission instantiation.
pub fn spawn_soldier(commands: &mut Commands, spawn: SoldierSpawn) -> Entity {
    let entity = commands
        .spawn((
            MissionScoped,
            Soldier {
                rank: spawn.rank,
                role: spawn.role,
            },
            Alive,
            Allegiance { side: spawn.side },
            Health {
                current: 100,
                max: 100,
            },
            Mobility { speed: 1 },
            Inventory {
                items: vec![Item {
                    kind: ItemKind::Ammo,
                    count: 120,
                }],
            },
            BattlefieldPosition(spawn.position_m),
            Heading(spawn.heading_radians),
            VisualSensor::default(),
            AuditorySensor::default(),
            EyeHeight::default(),
            SensorSignature::default(),
        ))
        .insert((
            Weapon::default_rifle(),
            CombatState::default(),
            Marksmanship::default(),
            CombatOrder::HoldFire,
            CombatOrderSource::doctrine(),
            PerceptionMemory::default(),
            VoiceComms,
            CommsLinks::default(),
            Inbox::default(),
            Outbox::default(),
            SeenPackets::default(),
            ReportCadence::default(),
            Autonomous,
        ))
        .insert((
            DecisionTrace::default(),
            PlannerBelief::default(),
            CommandPlanDelegationProgress::default(),
            DomainRef(DomainId::Soldier),
        ))
        .id();

    info!(
        "Soldier spawned! Rank: {:?}, Role: {:?}, Side: {:?}",
        spawn.rank, spawn.role, spawn.side
    );

    entity
}
