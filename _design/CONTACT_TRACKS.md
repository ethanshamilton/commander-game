# Contact Deduplication / Contact Tracks

Current `PerceptionMemory` stores one `Contact` per `(target, sensor kind)`, so the same entity can appear as separate visual and auditory contacts. That was useful while bootstrapping sensors, but it leaks duplication into rendering, reports, planning, and any UI that wants to show "known contacts".

Future refactor: store fused per-target tracks rather than raw duplicated contacts.

```rust
pub struct PerceptionMemory {
    pub tracks: Vec<ContactTrack>,
}

pub struct ContactTrack {
    pub target: Entity,
    pub contact_type: ContactType,
    pub observed_life_status: ReportedLifeStatus,
    pub last_known_position_m: Vec2,
    pub confidence: f32,
    pub last_observed_tick: u64,
    pub modalities: ContactModalities,
}

pub struct ContactModalities {
    pub visual: Option<ContactObservation>,
    pub auditory: Option<ContactObservation>,
    // radar/thermal/etc later
}

pub struct ContactObservation {
    pub position_m: Vec2,
    pub confidence: f32,
    pub observed_tick: u64,
}
```

Consumers should ask track-level questions:

- best current believed position
- last visual confirmation
- last auditory cue
- believed allegiance/type
- believed life status
- freshness/staleness

This would remove per-consumer dedup logic from battlefield rendering, reports, HTN planner synthesis, etc. Until then, consumers that need one contact per target should locally fold `PerceptionMemory.contacts` by target and choose the best observation, e.g. newer tick > higher confidence > visual over auditory.
