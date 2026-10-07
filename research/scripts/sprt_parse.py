#!/usr/bin/env python3
"""Parse cutechess-cli output on stdin into the `sprt` object of result.json."""
import json
import math
import re
import sys

SCORE = re.compile(r"Score of (\S+) vs (\S+): (\d+) - (\d+) - (\d+)")
ELO = re.compile(r"Elo difference: (\S+) \+/- (\S+?),")
SPRT = re.compile(r"^SPRT: llr (-?\d+(?:\.\d+)?)(?:.*? - (H[01]) accepted)?", re.M)


def _num(text):
    try:
        value = float(text)
    except ValueError:
        return None
    return value if math.isfinite(value) else None


def parse(text, candidate="new"):
    scores = SCORE.findall(text)
    if not scores:
        raise ValueError("no 'Score of' line: the match did not run")
    first, _second, wins, losses, draws = scores[-1]
    if first != candidate:
        raise ValueError(f"expected '{candidate}' listed first, got '{first}'")
    wins, losses, draws = int(wins), int(losses), int(draws)

    elo = ELO.findall(text)
    sprt = SPRT.findall(text)
    return {
        "games": wins + losses + draws,
        "wins": wins,
        "losses": losses,
        "draws": draws,
        "elo": _num(elo[-1][0]) if elo else None,
        "elo_err": _num(elo[-1][1]) if elo else None,
        "llr": float(sprt[-1][0]) if sprt else None,
        "verdict": sprt[-1][1] if sprt and sprt[-1][1] else "inconclusive",
    }


if __name__ == "__main__":
    try:
        print(json.dumps(parse(sys.stdin.read())))
    except ValueError as e:
        sys.exit(f"sprt_parse: {e}")
