# Do not try a null move straight after a null move

## Change
A node reached through a null move skips its own null-move pruning. The parent sets
`after_null[ply + 1]`; the child consumes the flag on entry, so none outlives a search.

## Mechanism
Two null moves in a row hand the move back and forth, so the second search cannot show anything
the first did not, and it costs nodes. Every mainstream engine forbids it.

## Expected Elo
Small: +0 to +10 from the nodes saved.
