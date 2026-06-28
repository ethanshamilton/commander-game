# NOTES.md

Running/evergreen notes file for the project.

## MAP
- Will need terrain types (i.e. grass, road, water)
- Will need a way to represent structures

## COMMAND

### HIERARCHY
- How should we represent the squads of units?
- When a unit is selected, we should display links up and down the chain of command one level. We should point to a unit's superior and have the subordinate units point up to the selected unit. These should be blue lines.

### REPORTING TO THE PLAYER
- The player is not omniscient in this game. The player must be in contact with units in order to receive information and issue commands.
- Two modes will eventually exist: direct command and remote command.
  - Direct command means the player is one of the units on the battlefield.
  - Remote command means the player is somehow in contact with the battlefield but depends on the means of communication available. For now we will not consider remote command.
- Units in direct contact with the player will be ACTIVE. Units out of contact with the player will be STALE.

### COMMUNICATIONS
- In order to receive information and issue commands, communications must be established between the source and target units.
- We will start with voice communication. If units are within auditory sensor range, then a communication link is established.
- Enemies may be able to intercept communications based on the communication type and sensor ranges.
  - For voice communication, if the enemy is in range of the source unit, then it has access to the source unit's intel.
