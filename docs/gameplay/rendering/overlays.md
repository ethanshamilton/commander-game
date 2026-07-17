# Tactical Overlays

Tactical overlays are contextual annotations on top of the battlefield view. They help reveal relationships, intentions, and uncertainty without changing the simulated state.

The selected unit's terrain-clipped sensor cone is cached. Its expensive line-of-sight geometry is recomputed after simulation ticks (and when selection, map, control, or player knowledge invalidates it), while cached line segments are submitted each render frame.
