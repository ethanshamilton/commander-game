# Info Packets

Info packets are typed messages that can be transmitted through the comms graph.
They are the content layer above communications: comms answers who can currently
hear whom, while packets describe what was said.

Packets are immutable after creation. Relays preserve the original `origin` and
payload verbatim; the system does not model telephone-game corruption or relay
chains. Payloads are claims/belief snapshots rather than ground truth. `StatusReport`
and `ContactReport` payloads feed player tactical knowledge after traveling
through delivery/relay. Order-command payloads are the exception in kind rather
than authority: they are social facts that are accepted only after
recipient-side command validation.

Delivery is one hop per simulation tick over direct comms links. A sender's
outbox is drained during the comms simulation set and each packet is copied into
each direct neighbor's inbox unless that neighbor has already seen the packet ID.
Addressing does not filter physical hearing; consumers and relay doctrine
interpret the address later.

Relay is explicit unit doctrine rather than free transport-layer routing. After
one-hop delivery, fresh inbox entries are evaluated: direct packets addressed to
another unit are moved unchanged to the relay unit's outbox, direct packets for
this unit remain in its inbox for consumers, and broadcast packets are kept and
also re-broadcast once. Because relay runs after delivery, relayed packets do not
transmit until the next comms tick.

Each unit accepts a given packet ID at most once and relays a fresh inbox entry
at most once, so flooding halts instead of looping forever.

Player micro-orders are carried as `OrderCommand` packets. When a packet reaches
its addressed unit, the recipient consumes the inbox entry, verifies that the
origin is the player-controlled command node and has authority in the command
forest, then installs the wrapped unit/combat order with player-sourced order
provenance. Failed authorization still consumes the packet; the network does not
retry or reinterpret it.

Task assignments and cancellations are also packet-carried intent. Recipients require a living current commander, valid plan identity, and a strictly newer issue tick. Cancellation is processed before revised assignment in a comms pass; accepted cancellation removes `AssignedTask`, abandons its runner, and clears only HTN-sourced concrete orders. Dead-predecessor and stale packets are consumed and rejected.

Inbox entries have a finite lifetime (`INBOX_TTL_TICKS`) and are pruned before
delivery each comms tick. `SeenPackets` is deliberately not pruned with the
inbox: if an old utterance still matters, a sender should create a new packet
with a new ID rather than rely on network-level retry.

Packet IDs are mission-runtime state. The allocator is reset when a mission is
spawned alongside the other mission runtime resources.
