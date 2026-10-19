# Differential perft

`tests/perft.rs` pins perft counts for a handful of positions. This directory checks move
generation against Stockfish on thousands of positions instead.

```bash
# 3000 positions from the opening book, each walked a random number of plies, perft(4):
shuf -n 3000 /root/UHO_Lichess_4852_v1.epd | \
  cargo run --release --example walk_perft -- 4 | python3 tests/differential/diff_perft.py 4
```

The walk uses a fixed seed, so the same input always yields the same positions. When it was
first run this way it compared 2,982 positions at depth 4 with no mismatches.
