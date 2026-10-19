# Razoring: drop to quiescence when the static eval is far below alpha

## Change
At low depth in non-PV nodes, if static eval plus a margin is still below alpha, search quiescence instead and return its result if it fails low.

## Mechanism
Positions far below alpha rarely recover; quiescence confirms it cheaply.
