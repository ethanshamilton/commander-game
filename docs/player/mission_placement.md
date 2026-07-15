# Tactical mission placement

`MissionPlacementState` owns the temporary map-click interaction used to create
tactical missions. While it is active, ordinary unit selection and contextual
micro-orders must not consume map clicks.

The initial Hold Line flow is three left-clicks in the playable map region:

1. line start;
2. line end;
3. rally point.

Coordinates are converted from Bevy render units to meters before a
`MissionPlan` is created. Escape cancels placement. A completed valid plan is
spawned as `TacticalMission + ScenarioScoped + MissionAssignees`, becomes the
selected tactical mission, and receives a generated label such as `Hold Line 1`.
