# Truth Pipeline

Commander separates what is true from what units know and what the player sees.

Ground truth is the authoritative ECS world state: the actual components and resources on entities. It is not a separate central database. Scenario data only provides initial conditions; after spawning, the ECS world is the source of truth.

The epistemic pipeline is:

```text
ECS Ground Truth
    -> Perception / observation
Unit-local memory
    -> Communications / reporting
Player tactical knowledge
    -> Rendering / UI
Player display
```

## Principles

- Simulation systems may read and mutate ground truth.
- Perception systems convert ground truth into unit-local memory.
- Reporting systems convert unit-local memory into player tactical knowledge through communications.
- Normal rendering and UI should use player tactical knowledge, not omniscient ground truth.
- Debug views may intentionally read ground truth, but should not contaminate player knowledge.

The goal is to preserve disciplined boundaries between what is true, what units know, what the player knows, and what the interface displays.
