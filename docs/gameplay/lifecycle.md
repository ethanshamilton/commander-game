# Unit Lifecycle

Unit lifecycle tracks the binary ground-truth distinction between living actors and dead entities.

Dead units are not immediately despawned. They remain in the ECS world so player knowledge can stay stale and living units can later observe the body. Death removes active capabilities such as movement orders, sensors, comms, links, and perception memory.
