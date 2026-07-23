# Missions

Missions are authored starting conditions for play. They bind a battlefield context to the initial actors and situation that the simulation will evolve.

Squads are authored as ordered lists of stable `UnitId`s. The first member is the initial leader and later members define command succession order. Runtime instantiation resolves the roster to entities, installs `MemberOfSquad` reverse links, and derives squad-internal `CommandForest` links from the roster.
