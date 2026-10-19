# SEE pruning: skip captures that lose material at low depth

## Change
Captures with a negative static exchange score are skipped at shallow depth in non-PV nodes.

## Mechanism
A capture that loses material in the exchange rarely changes the result at low depth.
