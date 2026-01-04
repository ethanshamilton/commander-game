# Commander
This is a 2D RTS game where you command a military force.

## Gameplay
You are at the top of the chain of command ordering your units to complete
the mission objective. There will be campaign level gameplay as well where
you build your force from the ground up by acquiring income, producing units,
and unlocking advanced technology.

## Systems
This game is about simulating complex systems in the battle such as communications,
logistics, morale, force management, and more. 

### Chain of Command
The chain of command is what determines the actions units and soldiers will take. Orders are issued by
soldiers of higher rank and executed by soldiers of lower rank. Orders become progressively tactical
the farther down the chain of command they reach, and more strategic the higher up they are issued. For
example, the highest ranking soldier can issue a command to a unit to take an area. The unit leader will 
then order the soldiers to scout areas, take positions, and watch over areas. Individual soldiers will
execute their orders to the best of their ability, or a unit leader can even decide to subdivide their unit
and designate sub-unit leaders to issue orders as well.

#### Rank List
- Enlisted
    - E1: Private
    - E2: Corporal
    - E3: Sergeant
    - E4: Sergeant Major
    - E5: Master Sergeant
- Officer
    - O1: Lieutenant
    - O2: Captain
    - O3: Major
    - O4: Colonel
    - O5: General

### Communications
To command your force, you have to be able to issue the orders. Communications systems enable the orders to
be issued. At the simplest level you have face to face communications. Advancing the tech tree can enable 
more advanced communications. If communications are intercepted by the opponent then orders will be
revealed. 

#### Communications Progression
1. Face-to-Face (audio): up to 10 meters, can be intercepted by any opponent in range.
2. Hand Signal (visual): up to 30 meters, can be intercepted by any opponent in line of sight and within range.
3. Paper Message (visual): up to 1 meter, must be delivered to recipient. 
4. Radio (audio): depends on tech level, can be intercepted by opponent with radio tech unless encrypted.

### Logistics
Soldiers will consume items such as ammo, medical supplies, grenades, etc. In order to continue fighting
they must be resupplied. 

### Intelligence
The player will not be guaranteed perfect knowledge of unit positions or status. Intelligence is decentralized,
meaning units that are not in contact with the player will have knowledge of their surroundings and be able
to make decisions to the best of their intelligence but this info will not be relayed to the player unless
contact is established. 
