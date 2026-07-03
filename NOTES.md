# NOTES.md

Running/evergreen notes file for the project.

## MAP
- Will need terrain types (i.e. grass, road, water)
- Will need a way to represent structures

## COMMAND

### HIERARCHY

## COMMUNICATIONS
- Enemies may be able to intercept communications based on the communication type and sensor ranges.
  - For voice communication, if the enemy is in range of the source unit, then it has access to the source unit's intel.
    - This turns out to be relatively complex in that we need a way to account for what it means to have intel be intercepted.

### INFO PACKETS
- A way to model the propagation of information through the comms graph may be to model "information packets". These would be things like contact reports, commands, whatever. That gives us units of information to interact with when calculating things like encryption and interception.
  - Info packets should probably be sent at some interval. A unit wouldn't be constantly reporting where its hostile contacts are to the leader, it would report them at like, every 10 seconds or something.

## CONTROLS
- Implement multi-unit select (shift-click and mouse rectangle)
  - In order for this to be useful when working with Move orders, we need to implement basic formations.
