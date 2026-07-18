# Tactical mission placement

`MissionPlacementState` owns the temporary map-click interaction used to create
tactical missions. While it is active, ordinary unit selection and contextual
micro-orders must not consume map clicks.

The initial Hold Line flow begins with an expiration duration in whole minutes,
then uses three left-clicks in the playable map region:

1. line start;
2. line end;
3. rally point.

The duration defaults to five minutes. Entering zero creates a mission with no
expiration. Nonzero durations are converted to an absolute simulation tick when
the rally point is placed, so time spent delivering the assignment does not
extend the mission.

Coordinates are converted from Bevy render units to meters before a
`MissionPlan` is created. Escape cancels placement. A completed valid plan is
spawned as `TacticalMission + ScenarioScoped + MissionAssignees`, becomes the
selected tactical mission, and receives a generated label such as `Hold Line 1`.
