You are attempting one engine-improvement hypothesis for the lemonate chess engine.

- Experiment: {{ID}}
- Work only in this git worktree: {{WORKTREE}} (branch `exp/{{ID}}`)
- Hypothesis file: {{EXPERIMENT}}/hypothesis.md. Fill in its three sections BEFORE editing any code.
- Read first: {{REPO}}/CLAUDE.md (engine architecture) and {{RESEARCH}}/CATALOG.md (results so far, so you do not retry rejected ideas).

Rules:

1. Edit files under `src/` only. Do not touch `tests/`, `research/`, `lean/`, `Cargo.toml`, `Cargo.lock`, `build.rs`, `.cargo/` or `rust-toolchain*`; changes there make the attempt `broken`.
2. While iterating, run `cargo test --release --lib --bins`. That is the only verification you run.
3. Do not run `research/scripts/gate.sh`, cutechess, or any engine-vs-engine match. The gate is run by someone else, so you cannot tune the patch against its results.
4. Commit your work on branch `exp/{{ID}}`. The gate measures the committed HEAD and refuses uncommitted changes.
5. Before finishing, write {{EXPERIMENT}}/trace.md with these sections:
   - `## Summary`: one line saying what you changed and why you expect it to help
   - `## Tried`: what you tried, in order
   - `## Dropped`: ideas you discarded and the reason
   - `## Surprises`: anything unexpected in the code or behaviour
6. Do not rebase, merge or cherry-pick `main` or any other branch into your branch. The gate rejects branches that do not descend from the recorded baseline.
7. In `{{EXPERIMENT}}/` write only `hypothesis.md` and `trace.md`. Never write `result.json`, `baseline_commit` or any other file there.
