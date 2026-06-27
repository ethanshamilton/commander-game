pub const BEVY_UNITS_PER_METER: f32 = 10.0;

pub const fn meters(value: f32) -> f32 {
    value * BEVY_UNITS_PER_METER
}

#[allow(dead_code)]
pub const fn to_meters(bevy_units: f32) -> f32 {
    bevy_units / BEVY_UNITS_PER_METER
}
