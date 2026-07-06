# Player Knowledge

Player knowledge is the player's operational picture of the battlefield. It deliberately mediates between simulation truth and what has been observed, reported, or inferred.

The player is treated as a node in the command/comms graph. The player directly knows the self-state and current perceptions of the player-controlled unit, but knowledge about other units is updated from report packets received in that unit's inbox.

Current report packet handling:

- friendly units author `StatusReport` packets to the player-controlled unit on their `ReportCadence` status interval, currently once per simulated second
- friendly units author `ContactReport` packets on their `ReportCadence` contact interval, currently once per simulated second, for contacts observed recently enough to avoid cadence/tick aliasing
- reports travel through the info-packet delivery/relay system before the player can consume them
- the player consumes status/contact report packets from the player-controlled unit inbox and removes those entries
- non-report packets in the inbox are retained for their own consumers

This replaces direct tactical-knowledge reads over `CommsGraph` reachability: the comms graph now determines whether reports can physically travel, not what the player instantly knows.

Merge semantics distinguish observation freshness from receipt freshness. A newly received stale report can update `last_reported_tick`, but it must not overwrite a more recently observed position/life-status snapshot. `last_observed_tick` owns battlefield-fact freshness; `last_reported_tick` owns comms receipt freshness.
