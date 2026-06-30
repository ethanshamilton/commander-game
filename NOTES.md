# NOTES.md

Running/evergreen notes file for the project.

## MAP
- Will need terrain types (i.e. grass, road, water)
- Will need a way to represent structures

## COMMAND

### HIERARCHY
- How should we represent the squads of units?
- When a unit is selected, we should display links up and down the chain of command one level. We should point to a unit's superior and have the subordinate units point up to the selected unit. These should be blue lines.

### COMMUNICATIONS
- Enemies may be able to intercept communications based on the communication type and sensor ranges.
  - For voice communication, if the enemy is in range of the source unit, then it has access to the source unit's intel.
    - This turns out to be relatively complex in that we need a way to account for what it means to have intel be intercepted.

## NEEDS FIX
- There are currently two notions of position, one in units.rs and the other is BattlefieldPosition. Need to reconcile.
