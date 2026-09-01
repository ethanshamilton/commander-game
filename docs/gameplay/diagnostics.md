# Gameplay Diagnostics

Diagnostics expose the health of the simulation itself. This module is for observing whether the model is running acceptably, without changing gameplay semantics.

## Tick timing

`SimulationPerf` measures wall-clock time for each fixed-update simulation tick and reports utilization against the tick budget (`1 / SIMULATION_TICK_HZ`).

## Per-phase profile

Because the simulation sets are chained (strictly sequential), tiny boundary systems between each consecutive pair of `SimulationSet`s can attribute wall time to each phase exactly. Each phase's duration is folded into an exponential moving average (α = 0.05, roughly a one-second window at 20Hz), giving an aggregate view of which phases dominate the tick budget. Measured time includes scheduler overhead within a set, which is a real cost.

The mission screen shows the tick meter in the top-right diagnostics box; clicking that box toggles the per-phase breakdown, sorted most expensive first.

## Command succession

`CommandSuccessionDiagnostics` counts succession attempts, successful assumptions, orphaned commands, redelegation resets, cancellation packets issued/applied, and stale or dead-origin command packets rejected. These mission-scoped counters reset when entering a mission and are observational only.

## Frame timing

Bevy's frame-time diagnostic supplies a smoothed FPS measurement. The mission screen's `RENDER` meter shows current FPS as a percentage of a 60 FPS target. This reflects whole-frame pacing and is not a GPU-only measurement.
