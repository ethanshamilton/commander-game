/// Reported/observed life status as it appears in intel.
///
/// This is deliberately distinct from ground-truth lifecycle markers like
/// `Alive`/`Dead`: reports can be stale, wrong, absent, or propagated through
/// comms before reaching a consumer such as the player's tactical picture.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportedLifeStatus {
    Alive,
    Dead,
    Unknown,
}
