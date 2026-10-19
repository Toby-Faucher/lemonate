#!/usr/bin/env python3
"""Compare lemonate's perft with Stockfish's on random-walk positions.

Reads `FEN<TAB>moves<TAB>count` lines (from examples/walk_perft.rs) on stdin, asks Stockfish
for `go perft DEPTH` on the same position, and prints every mismatch. Exit status is 1 if any
position disagrees, so it can gate a CI job.

    cargo run --release --example walk_perft -- 4 < fens.txt | python3 diff_perft.py 4 [stockfish]

Needs a `stockfish` binary on PATH (or pass its path). Any FEN list works; the UHO opening
book used by the research gate gives a good spread.
"""
import subprocess
import sys


def main():
    depth = int(sys.argv[1])
    engine = sys.argv[2] if len(sys.argv) > 2 else "stockfish"
    sf = subprocess.Popen([engine], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)

    def stockfish_perft(fen, moves):
        position = f"position fen {fen}" + (f" moves {moves}" if moves else "")
        sf.stdin.write(f"{position}\ngo perft {depth}\n")
        sf.stdin.flush()
        for line in sf.stdout:
            if line.startswith("Nodes searched"):
                return int(line.split(":")[1])
        raise SystemExit("stockfish exited unexpectedly")

    checked = bad = 0
    for line in sys.stdin:
        parts = line.rstrip("\n").split("\t")
        if len(parts) != 3:
            continue
        fen, moves, ours = parts[0], parts[1], int(parts[2])
        checked += 1
        theirs = stockfish_perft(fen, moves)
        if theirs != ours:
            bad += 1
            print(f"MISMATCH {fen} | moves {moves or '-'} | lemonate {ours} | stockfish {theirs}")
    sf.stdin.write("quit\n")
    sf.stdin.flush()
    print(f"checked {checked} positions, {bad} mismatches")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
