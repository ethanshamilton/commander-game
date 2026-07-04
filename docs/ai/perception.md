# AI Perception

Perception models what an actor can sense about the world. It is intentionally separate from global truth: agents should make decisions from locally available, imperfect information.

## Scan cadence

Expensive sensor geometry is intentionally staggered. Each observer performs visual and auditory scans on a deterministic 5-tick cadence, offset by entity id, so a 20Hz simulation gives each observer an effective 4Hz scan rate while spreading work across ticks.

Contacts are still cheaply refreshed every tick when nothing relevant has been rechecked. This preserves the downstream contract that `last_seen_tick == clock.tick` means "currently perceived" for combat and comms.

## Change stamps

`SensorStamp` records the last simulation tick where a unit changed in a perception-relevant way: position, heading, signature, or death. On an observer's scan tick, unchanged observer-target pairs are skipped; changed pairs are re-evaluated. This avoids repeated line-of-sight checks for stationary standoffs while accepting a small perception latency window.
