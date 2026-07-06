# Selection

Selection translates player pointing and command gestures into references to known battlefield entities. It is an interaction layer over the player's tactical picture.

Known units remain selectable even when their reports are stale. Issuing an order to a selected friendly unit does not require current tactical knowledge or current comms reachability: the player authors intent, then the info-packet system determines whether the order packet actually reaches the recipient.

The player-controlled unit remains a special micro-control case. Orders addressed to that unit are applied directly; orders addressed to subordinate friendlies are sent as `OrderCommand` packets.
