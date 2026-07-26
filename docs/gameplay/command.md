# Command

The command module models superior/subordinate relationships as a runtime forest of individual units. The forest is initialized from stable mission data IDs and then keyed by Bevy entities once a mission is spawned.

The command forest is separate from the comms graph: command authority answers who is allowed to command whom; comms answers whether orders or reports can currently travel.

The forest stores each edge in both superior and subordinate maps. `CommandForest::validate` checks that these views agree, every referenced node exists, subordinate lists contain no duplicates, and the graph contains neither self-command nor cycles.

`CommandForest::succeed` atomically removes a command node and can promote one of its direct children. The successor takes the deceased node's place beneath its former superior, while the deceased node's other direct children transfer beneath the successor in their existing order. Existing deeper subtrees are preserved. If there is no successor, direct children become roots. Mutation is prepared and validated on a copy, so an invalid request leaves the current forest unchanged.

Authority has two forms. `can_command_in_forest` answers the topology-only question and may be used for organization views. Executable orders and assignments use `can_issue_command`, which additionally requires both issuer and recipient to be living. This prevents delayed packets from dead commanders from exercising stale authority.
