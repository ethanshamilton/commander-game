# Map Rendering

Map rendering presents the battlefield substrate to the player. It visualizes the space that the simulation occurs in without owning that space.

Terrain contour segments are computed in battlefield meters when the map changes and cached thereafter. Drawing still submits those cached segments through gizmos each frame, leaving persistent meshes as a possible later optimization.
