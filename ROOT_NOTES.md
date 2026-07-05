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

## AI
- Still struggling to understand the HTN system a bit. Overall concepts make sense but implementation was done quickly and I need time for it to sink in.
  - I need to be able to intuit how given domains need to change when game mechanics are introduced so that I can effectively develop AI behaviors.
  - I wonder if there's a way to "learn" the domain for an HTN. Right now plan value is determined by how I order the tasks in the domain, but the best way to learn plan values would be through ML.
- Want to implement trace viewer so we can start to analyze how AI units make decisions
- TTK is extremely fast when enemy AI spots friendly units. Need to start working on delegation of actions i.e. fire at will to friendly units.
- Need to start figuring out how to implement mission system. This will support higher levels of delegation so that we aren't trying to micromanage AI unit response times.

## COMBAT

### INFANTRY MECHANICS
- Need to add stance (standing, crouch, prone) with corresponding effects on accuracy
  - Standing reduces target unit's accuracy, makes it easier for enemies to hit (enemy unit's base accuracy value fully applies)
  - Crouching increases target unit's accuracy, decreases move speed slightly. Reduces enemy unit's accuracy.
  - Prone increases target unit's accuracy the most but narrows engagement cone and significantly reduces move speed. Significantly reduces enemy unit's accuracy.

## TOOLS

### MISSION EDITOR
- Would be useful to have a mission editor which allows units/squads to be placed and set victory conditions etc. Just something to set the initial state of a mission.

### MAP EDITOR
- Right now map design is kind of inscrutable because it's just implemented with a hardcoded height map. If there were a way to create these with a tool, that would be nice.
