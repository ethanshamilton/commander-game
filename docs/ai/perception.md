# AI Perception

Perception models what an actor can sense about the world. It is intentionally separate from global truth: agents should make decisions from locally available, imperfect information.

Visual and auditory sensing use two phases:

1. The dense `BattlefieldSpatialGrid` returns entities from cells intersecting the sensor range's bounding box.
2. Perception performs exact narrow-phase checks: self/life/signature filtering, distance and field of view, and terrain line of sight where applicable.

The spatial grid is conservative and may return entities outside the circular sensor range. It changes which pairs are worth examining, not the meaning of visibility or audibility. Corpses remain visual candidates but are rejected by auditory sensing.

Visual LOS samples terrain height every two meters between observer and target. This remains the expensive narrow phase; the spatial index reduces how often it is invoked but does not cache or otherwise optimize individual LOS walks.
