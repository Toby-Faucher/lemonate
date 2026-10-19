# Reserve 10 ms per move for GUI latency (Move Overhead)

## Change
New UCI option `Move Overhead` (default 10 ms, 0-5000) is subtracted from a fixed `movetime` and
from the remaining clock before the game-clock budget is computed. Also adds `Clear Hash` and
streams `info` after every iteration; neither changes search decisions.

## Mechanism
GUI and pipe latency eat into the clock; without a reserve the engine can lose on time at fast
controls. A 10 ms default is small enough to cost little strength at 10+0.1.

## Expected Elo
Slightly negative at this time control (about 4 percent less thinking time per move). Tested with
non-regression bounds [-4, 1]: the purpose is safety, not strength.
