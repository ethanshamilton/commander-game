# Formations

A formation is pure coordinator-relative geometry generated for an arbitrary
number of participants. `FormationSpec` defines an anchor, facing, spacing, and
shape; `generate_formation_positions` returns exactly one absolute position per
participant, with position zero reserved for the coordinator.

Formation geometry does not move units and does not maintain a centralized
formation controller. A leader's HTN assigns generated positions to members and
sends those positions as task directives through the normal communications
substrate. Formation assignments are `PositionTarget` poses: every member gets
both an absolute station and the formation heading. Recipients independently
turn their assigned pose into concrete HTN movement and hold orders.

The initial shape is a wedge. It grows by adding successively wider rows behind
the coordinator, so it has no hard-coded squad-size limit. Hold Line
coordination uses the mission rally point as the wedge anchor and faces the
wedge back toward the held line. Unique fallback positions and their shared
heading are bundled into the original subordinate task directives, allowing
expiry fallback to work after communications are lost. On arrival, movement
applies that heading before the unit begins holding.
