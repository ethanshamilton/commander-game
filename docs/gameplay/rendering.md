# Gameplay Rendering

Gameplay rendering turns simulation state into a tactical display. It should be a projection of the model, not a second source of truth.

Normal unit rendering uses `PlayerTacticalKnowledge`, not live battlefield truth, for known unit positions. Friendly units outside the player unit's direct perception therefore move stepwise as reports arrive. Hostile units known through reports are rendered as knowledge-backed hostile glyphs/contact markers at their last reported position; this does not imply omniscient access to their current ground-truth position.

Rendering uses report/observation recency rather than exact same-tick freshness. Exact-tick checks are too brittle now that reports travel as throttled info packets.
