# Capture history: order captures by a learned [piece][to][captured] score on top of MVV-LVA/SEE

## Change
Add a capture history table [color][attacker piece][to square][captured piece type] to `HistoryTable`. Captures are scored `GOOD_CAPTURE + MVV-LVA + capthist/64` (and bad captures get the same term). On a capture beta cutoff the cutoff capture gets a depth-squared bonus; captures tried earlier in the node that failed to beat alpha get a malus. On a quiet cutoff the failed captures also get the malus.

## Mechanism
MVV-LVA/SEE ignore position-specific knowledge. Capture history learns which captures actually cut off (or fail) in this search, so among captures of similar value the proven ones are tried first and refuted ones last. Earlier cutoffs mean fewer nodes per depth.

## Expected Elo
+8 to +15 (per issue #82).
