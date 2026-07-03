# Spatial

Core spatial state for simulation entities.

`BattlefieldPosition` is the single runtime position component for battlefield entities. It stores horizontal battlefield coordinates in meters. Terrain height is derived from the active map terrain rather than stored directly on actors.

Future air or elevated units should extend this spatial model with altitude rather than adding a parallel actor position component.
