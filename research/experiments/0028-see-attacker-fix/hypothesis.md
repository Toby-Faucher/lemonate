# Fix SEE removing the wrong attacker

## Change
After choosing the cheapest attacker, remove that piece's own bit from the occupancy, not the lowest set bit of all attackers. Adds a unit test.

## Mechanism
A correctness fix. SEE with two or more defenders of different types treated the wrong piece as used, so exchanges were misjudged.
