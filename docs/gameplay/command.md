# Command

The command module models superior/subordinate relationships as a runtime forest of individual units. The forest is initialized from stable scenario data IDs and then keyed by Bevy entities once a scenario is spawned.

The command forest is separate from the comms graph: command authority answers who is allowed to command whom; comms answers whether orders or reports can currently travel.
