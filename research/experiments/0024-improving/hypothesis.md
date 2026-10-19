# Improving heuristic: scale reverse futility and LMR by whether the eval is rising

## Change
Track the static eval per ply. When it is higher than two plies earlier the node is improving: reverse futility is less eager and LMR reduces less; otherwise the reverse.

## Mechanism
A rising eval means the side to move is doing well, so it deserves more search; a falling one deserves less.
