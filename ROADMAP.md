# Roadmap

## Now
- PLANS: Plan system for use by both AI and human commanders with multiple plan types, allowing macro level command
- COMMAND: Command succession and dynamic redelegation.
- MISSIONS: Multi-squad command tutorial mission.
  - This will be used to implement and test command succession.
- CODE: Split up `src/screens/missions.rs` as it is becoming a god module.

## Next
- MAP: Terrain types, i.e. water, road
- MAP: Structures, i.e. tree, wall
- UI: View the entire command tree the player has control over. Command tree view.
- COMMAND: Squad organization, more than just superior/subordinate. An abstraction for squads themselves.
- COMBAT: Stances, i.e. standing/crouching/prone.
- AI: Basic search pattern for soldiers when holding position.
- TOOLS: Data-driven missions and maps. Get them out of source code.
- TOOLS: Tuning resource. Right now there are a lot of constants for tuning around the codebase; consolidate them.
- DEV: Tune dev profile for faster compile times.
- DIAGNOSTICS: Try `bevy-inspector-egui` for live inspection.

## Later
- COMMS: Intercept communications from opponents.
- TOOLS: Map editor
- TOOLS: Mission editor
