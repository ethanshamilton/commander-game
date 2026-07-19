# Simulation

The simulation module is the temporal spine of gameplay. It defines how the tactical model advances over time and gives other systems an ordered place to participate in that advance.

The fixed-update phases are chained in this order:

1. Clock
2. Orders
3. Movement
4. SpatialIndex
5. Sensors
6. Comms
7. Reports
8. Thinking
9. Combat
10. Objectives
11. Cleanup

The spatial index is rebuilt from post-movement `BattlefieldPosition` values, so every downstream phase observes one coherent position snapshot for the current tick.

Movement destinations use `PositionTarget`, which pairs a position with an
optional arrival heading. Units face their direction of travel en route. On
arrival, movement applies the requested heading before completing the order;
position-only targets preserve the existing arrival heading.
