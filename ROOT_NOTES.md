# NOTES.md

Running/evergreen notes file for the project.

## MAP
- Will need terrain types (i.e. grass, road, water)
- Will need a way to represent structures

## GRAPHICS

## COMMAND

### HIERARCHY
- Eventually want to create a view in the left sidebar that shows the player's command tree. It shouldn't show the whole command forest, just the units and squads that the player has control over. Player can use that to select units or squads.

### MISSIONS
- The main intended way for players to interact with their units is via the assignment of missions. Any micromanagement, in other words, the direct assignment of orders, is a failure mode in the ideal state of this game. We need to be able to assign missions like "hold a line from (A,B) to (X,Y)" and then the player can assign squads to a mission and the squad leader becomes responsible for figuring out how to implement the orders to accomplish the mission.
- Starter Mission types
  - Hold Line
    - Commander defines a line from (A,B) to (X,Y) and assigned squads maneuver to cover the line.
  - Secure Perimeter
    - Commander defines a circle or rectangle and assigned squads maneuver to cover the perimeter.
  - Scout Area
    - Commander defines a point, circle, or rectangle and assigned squads manuever to search the area. If any hostiles are identified, priority is to disengage and report information up the chain of command.
  - Clear Area
    - Commander defines a point, circle, or rectangle and assigned squads maneuver to search the area and engage any identified hostiles.

## COMMUNICATIONS
- Enemies may be able to intercept communications based on the communication type and sensor ranges.
  - For voice communication, if the enemy is in range of the source unit, then it has access to the source unit's intel.
    - This turns out to be relatively complex in that we need a way to account for what it means to have intel be intercepted.

### INFO PACKETS

## CONTROLS
- Implement multi-unit select (shift-click and mouse rectangle)
  - In order for this to be useful when working with Move orders, we need to implement basic formations.

## AI
  - I need to be able to intuit how given domains need to change when game mechanics are introduced so that I can effectively develop AI behaviors.
  - I wonder if there's a way to "learn" the domain for an HTN. Right now plan value is determined by how I order the tasks in the domain, but the best way to learn plan values would be through ML.
- TTK is extremely fast when enemy AI spots friendly units. Need to start working on delegation of actions i.e. fire at will to friendly units.

### SOLDIER DOMAIN
- Hold behavior should be updated to a routine search pattern. Unit simply rotates slowly to examine surroundings.

## COMBAT

### INFANTRY MECHANICS
- Need to add stance (standing, crouch, prone) with corresponding effects on accuracy
  - Standing reduces target unit's accuracy, makes it easier for enemies to hit (enemy unit's base accuracy value fully applies)
  - Crouching increases target unit's accuracy, decreases move speed slightly. Reduces enemy unit's accuracy.
  - Prone increases target unit's accuracy the most but narrows engagement cone and significantly reduces move speed. Significantly reduces enemy unit's accuracy.

## TOOLS

### SCENARIO EDITOR
- Would be useful to have a scenario editor which allows units/squads to be placed and set victory conditions etc. Just something to set the initial state of a scenario.

### MAP EDITOR
- Right now map design is kind of inscrutable because it's just implemented with a hardcoded height map. If there were a way to create these with a tool, that would be nice.
