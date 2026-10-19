## Summary
Non-regression check for the 10 ms Move Overhead default: stopped by hand after 2,330 games at about +5 Elo, so it costs no measurable strength at 10+0.1.

## Tried
1. Added `Move Overhead` (default 10 ms), taken off a fixed movetime and off the remaining clock before the game-clock budget is computed. The same branch adds `Clear Hash` and per-iteration `info`; neither changes search decisions.
2. Ran an SPRT with non-regression bounds (-4, +1) against the old main on the local runner.

## Dropped
Nothing. The point of the test was safety, not strength, and an estimate of +5 after 2,330 games shows no loss larger than a few Elo.

## Surprises
A positive estimate for a change that only shrinks the time budget. It is noise (the 95% interval is roughly +/-12 at this game count), not a gain.
