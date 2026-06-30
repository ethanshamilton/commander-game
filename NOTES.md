# NOTES.md

Running/evergreen notes file for the project.

## MAP
- Will need terrain types (i.e. grass, road, water)
- Will need a way to represent structures

## COMMAND

### HIERARCHY
- When a unit is selected, we should display links up and down the chain of command one level. We should point to a unit's superior and have the subordinate units point up to the selected unit. These should be blue lines.

## COMMUNICATIONS
- Enemies may be able to intercept communications based on the communication type and sensor ranges.
  - For voice communication, if the enemy is in range of the source unit, then it has access to the source unit's intel.
    - This turns out to be relatively complex in that we need a way to account for what it means to have intel be intercepted.

### INFO PACKETS
- A way to model the propagation of information through the comms graph may be to model "information packets". These would be things like contact reports, commands, whatever. That gives us units of information to interact with when calculating things like encryption and interception.
  - Info packets should probably be sent at some interval. A unit wouldn't be constantly reporting where its hostile contacts are to the leader, it would report them at like, every 10 seconds or something.

## NEEDS FIX
- There are currently two notions of position, one in units.rs and the other is BattlefieldPosition. Need to reconcile.
