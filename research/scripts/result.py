#!/usr/bin/env python3
"""Assemble experiments/<id>/result.json from gate outcomes."""
import argparse
import json
from pathlib import Path

STAGES = ("preflight", "build", "test", "perft", "sprt")
CORRECTNESS = ("build", "test", "perft")


def derive_status(gate, verdict, infra_error):
    """Preflight violations and infrastructure failures are `broken`; a candidate that
    fails build/test/perft is `rejected`; otherwise the SPRT verdict decides."""
    if infra_error or gate.get("preflight") == "fail":
        return "broken"
    if any(gate.get(stage) == "fail" for stage in CORRECTNESS):
        return "rejected"
    return {"H1": "accepted", "H0": "rejected", "inconclusive": "inconclusive"}.get(verdict, "broken")


def build_result(baseline, candidate, config_hash, gate, sprt=None, reason="", infra_error=False):
    full = {stage: gate.get(stage, "skipped") for stage in STAGES}
    verdict = sprt["verdict"] if sprt else None
    return {
        "baseline_commit": baseline,
        "candidate_commit": candidate,
        "config_hash": config_hash,
        "gate": full,
        "sprt": sprt,
        "status": derive_status(full, verdict, infra_error),
        "reason": reason,
    }


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--out", required=True)
    p.add_argument("--baseline", required=True)
    p.add_argument("--candidate", required=True)
    p.add_argument("--config-hash", required=True)
    p.add_argument("--gate", action="append", default=[], metavar="STAGE=VALUE")
    p.add_argument("--sprt-file")
    p.add_argument("--reason", default="")
    p.add_argument("--infra-error", action="store_true")
    a = p.parse_args()

    gate = dict(item.split("=", 1) for item in a.gate)
    sprt = json.loads(Path(a.sprt_file).read_text()) if a.sprt_file else None
    data = build_result(a.baseline, a.candidate, a.config_hash, gate, sprt, a.reason, a.infra_error)
    Path(a.out).write_text(json.dumps(data, indent=2) + "\n")


if __name__ == "__main__":
    main()
