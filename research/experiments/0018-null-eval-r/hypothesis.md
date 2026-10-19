# Scale the null-move reduction by how far the eval is above beta

## Change
`R = 2 + depth/6` becomes `R = 2 + depth/6 + clamp((static_eval - beta) / 200, 0, 3)`.

## Mechanism
A position far above beta can afford a shallower verification search; one that only just reaches
beta keeps the old depth. Deeper reductions in clearly winning positions save nodes where the
cutoff is least likely to be wrong.

## Expected Elo
Small: +0 to +15, with some risk of missing tactics in positions with a large static advantage.
