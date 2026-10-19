# A guided tour of Lemonate

This guide explains the engine in the order you need to understand it, using the real code and
the numbers measured on this project. Each part ends with something to **try** and, at the
end, there are exercises. The aim is that you could explain every part of the engine to someone
else.

How to use it: read a part, open the files it names, run the "try it" commands, then do the
matching exercise before moving on. Write the exercise code yourself. Ask for a review, not for
the answer.

> Everything here describes `main`. Changes that are still open as pull requests are marked
> with their number so you can read them as worked examples.

## Contents

1. [The big picture](#1-the-big-picture)
2. [The board: bitboards and hashing](#2-the-board-bitboards-and-hashing)
3. [Move generation and perft](#3-move-generation-and-perft)
4. [Search: negamax and alpha-beta](#4-search-negamax-and-alpha-beta)
5. [Making the search faster by searching less](#5-making-the-search-faster-by-searching-less)
6. [Quiescence search](#6-quiescence-search)
7. [Evaluation](#7-evaluation)
8. [Tuning the evaluation](#8-tuning-the-evaluation)
9. [UCI, threads and time](#9-uci-threads-and-time)
10. [Measuring: how you know a change is good](#10-measuring-how-you-know-a-change-is-good)
11. [Traps this project has already fallen into](#11-traps-this-project-has-already-fallen-into)
12. [Exercises](#12-exercises)
13. [Reading list and glossary](#13-reading-list-and-glossary)

---

## 1. The big picture

A chess engine does two things:

1. **Search**: look ahead through the moves that could be played.
2. **Evaluate**: put a number on a position at the end of the look-ahead (positive: good for the
   side to move).

Everything else exists to make those two faster or better. Roughly: the search picks the move
that leads to the best evaluation that *both* sides can force.

```text
UCI text  ->  uci/            parse commands, run the search on a worker thread
                |
                v
             search/          negamax + alpha-beta, with all the tricks to search less
              |      |
              v      v
          board/    eval/     board: pieces, moves, hashes.   eval: score a position
          magic/              magic: fast attack lookups for sliding pieces
```

| Directory | What lives there |
|---|---|
| `src/bitboard`, `src/types` | the `Bitboard` and `Square` types, pieces and colours |
| `src/board` | the `Board`, FEN, make/unmake, move generation, Zobrist hashing |
| `src/magic` | magic bitboards: attack tables for bishops, rooks and queens |
| `src/search` | negamax, move ordering, transposition table, time management, Lazy SMP |
| `src/eval` | piece-square tables, pawn structure, king safety, mobility, game phase |
| `src/uci` | the text protocol and the thread that runs the search |
| `src/bin/tuner` | the program that fits the evaluation weights to game results |
| `tests/` | perft counts and end-to-end UCI tests |
| `research/`, `bench/` | the tools that measure strength and speed (part 10) |

**Try it** (build once, then talk to the engine):

```bash
cargo build --release
( echo uci; echo isready; echo "position startpos moves e2e4"; echo "go depth 8"; sleep 4 ) \
  | ./target/release/lemonate
```

The `sleep` matters: the engine stops searching when its input closes.

---

## 2. The board: bitboards and hashing

**Files:** `src/types/square.rs`, `src/bitboard/core.rs`, `src/board/mod.rs`, `src/board/fen.rs`,
`src/board/zobrist.rs`.

### Squares and bitboards

A `Square` is a number 0 to 63: `index = rank * 8 + file`, so a1 is 0, h1 is 7, a8 is 56 and h8
is 63.

```text
 8 | 56 57 58 59 60 61 62 63
 7 | 48 49 50 51 52 53 54 55
 6 | 40 41 42 43 44 45 46 47
 5 | 32 33 34 35 36 37 38 39
 4 | 24 25 26 27 28 29 30 31
 3 | 16 17 18 19 20 21 22 23
 2 |  8  9 10 11 12 13 14 15
 1 |  0  1  2  3  4  5  6  7
     a  b  c  d  e  f  g  h
```

A `Bitboard` is just a `u64` where bit *n* means "something is on square *n*". One bitboard per
piece type per colour answers "where are the white knights?" in one machine word. Set algebra
becomes CPU instructions:

| Question | Operation |
|---|---|
| squares occupied by either side | `white \| black` |
| enemy pieces a knight attacks | `knight_attacks & enemy` |
| is a square occupied? | `bb & (1 << sq) != 0` |
| how many pieces? | `count_ones()` (one instruction with `popcnt`) |

The `Board` (`src/board/mod.rs`) keeps `piece_bitboards[colour][piece]`, a bitboard per colour,
`all_pieces`, a `mailbox` (an array answering "what is on square *s*?"), whose turn it is, castling
rights, the en passant square and the two move counters. The bitboards are fast for sets; the
mailbox is fast for single squares. Both must always agree, which is why make and unmake are
written very carefully.

### Zobrist hashing

To recognise a position you have seen before you need a number that identifies it. Zobrist
hashing gives each (piece, square) pair, each castling right, each en passant file and "black to
move" its own random 64-bit key, and the position's hash is the XOR of the keys that apply
(`src/board/zobrist.rs`; the keys come from a fixed xorshift sequence built at compile time).

The trick is that XOR undoes itself. Moving a knight from b1 to c3 updates the hash with three
XORs (remove it from b1, add it to c3, flip the side to move) instead of rebuilding it. The
hash is used by the transposition table (part 5) and by repetition detection (part 5).

**Try it:** open `execute_move` in `src/board/make_move.rs` and find every place the hash is
updated. Then notice `undo_move` restores the old hash from a saved value rather than
recomputing it.

---

## 3. Move generation and perft

**Files:** `src/board/moves.rs`, `src/board/make_move.rs`, `src/magic/`, `tests/perft.rs`.

### Pseudo-legal and legal moves

The generator first makes **pseudo-legal** moves: every move the piece could make by its own
rules, ignoring whether the move leaves its own king attacked. Then it keeps only the **legal**
ones. The expensive part is that legality test: for each move, make it, ask "is my king
attacked?", unmake it.

Walking pieces (knights, kings, pawns) use small precomputed tables. Sliding pieces (bishops,
rooks, queens) depend on what blocks them, which is where **magic bitboards** come in.

### Magic bitboards

For a rook on e4, the only squares that can change where it stops are the rank and file squares
between it and the edge (the **mask**). Whatever sits on those squares (the **blockers**) decides
the attack set. Multiplying the blockers by a carefully chosen 64-bit number (the **magic**) and
shifting right turns them into a small index:

```text
index = ((blockers & mask) * magic) >> shift      // see Magic::hash in src/magic/mod.rs
attacks = table[offset + index]
```

The tables are filled once at start-up. The magics themselves are constants in `src/magic/mod.rs`;
`find_magic` can search for new ones, and tests prove that every constant puts no two blocker sets
with different attacks in the same slot.

### Perft: the move generator's test

**Perft** counts every leaf of the move tree to a fixed depth. For the starting position the
numbers are famous: 20, 400, 8,902, 197,281, 4,865,609. If your generator disagrees on any
position at any depth, it has a bug, and the position tells you where.

`tests/perft.rs` pins these counts for several tricky positions. This project also compared
perft with Stockfish on ~3,000 random positions. A move generator that passes this is almost
certainly correct, which is why every speed-up in generation can be checked exactly.

**Try it:**

```bash
cargo test --release --test perft
```

Then open `tests/perft.rs`, change an expected number by one, and watch the failure message.

---

## 4. Search: negamax and alpha-beta

**File:** `src/search/mod.rs`, functions `iterative_deepening`, `negamax`.

### Minimax and negamax

Chess is a two-player zero-sum game, so one number describes a position: how good it is for the
side to move. My best move is the one that gives the opponent the *worst* result:

```text
negamax(position, depth):
    if depth == 0: return evaluate(position)
    best = -infinity
    for each move:
        score = -negamax(position after move, depth - 1)    // the minus sign is the whole trick
        best = max(best, score)
    return best
```

The minus sign flips the point of view at each ply. That one line replaces separate "maximising"
and "minimising" code.

The cost: with about 35 moves per position, depth 6 means about 35^6 ≈ 1.8 billion leaves.

### Alpha-beta pruning

Suppose you have found a move worth +100 and are now looking at an alternative. If the opponent
can answer the alternative with something that leaves you at +20, you can stop looking at that
alternative immediately; you will never choose it. Alpha-beta tracks two bounds:

- **alpha**: the best score I am already guaranteed.
- **beta**: the best score the opponent will allow me; if I find a move at or above it, the
  opponent would have avoided this whole line earlier, so I can stop (a **beta cutoff**).

```text
negamax(position, depth, alpha, beta):
    for each move:
        score = -negamax(child, depth - 1, -beta, -alpha)
        if score >= beta:  return score          // cutoff
        alpha = max(alpha, score)
```

With the best move searched first at every node, alpha-beta examines about the square root of the
nodes plain minimax would: depth 12 becomes affordable. With the *worst* order it saves nothing.
So **everything in part 5 is about getting good moves searched first** or proving a move is bad
more cheaply.

### Iterative deepening

The engine does not search to depth 12 at once. It searches depth 1, then 2, then 3, and so on.
That sounds wasteful but is the opposite: each shallow search leaves its best move in the
transposition table, which makes the next, deeper search order its moves better. It also means a
usable answer always exists when time runs out. See `iterative_deepening`.

### Aspiration windows

At depth 5 and beyond, the search starts with a narrow window around the previous score
(`ASPIRATION_WINDOW = 50` centipawns). A narrow window prunes more. If the real score falls
outside it, the window is widened and the search repeats (`aspiration_search`).

**Try it:** set `ASPIRATION_MIN_DEPTH` to a large number in `src/search/mod.rs` to switch
aspiration off, and compare time to depth 12 with `bench/bench.py compare` (part 10).

---

## 5. Making the search faster by searching less

Every technique below is a gamble: it saves nodes at the risk of missing something. The gamble
is worth it when it wins more than it loses, and the only way to know is to measure (part 10).

### The transposition table (TT)

Different move orders reach the same position (e.g. 1.Nf3 Nf6 2.Nc3 and 1.Nc3 Nf6 2.Nf3). The TT
is a big array indexed by the Zobrist hash, storing for each position the score, the depth it
was searched to, whether the score is exact or only a bound, and the best move
(`src/search/transposition.rs`).

At the top of `negamax` the engine looks the position up:

- If the stored search is deep enough and its score decides the window, return it at once
  (a **TT cutoff**, only in non-PV nodes).
- Otherwise use the stored best move as the **hash move**: search it first.

The table has a fixed size (`Hash` option, default 64 MB, always a power of two so the index is a
mask). When two positions land in the same slot the newer one replaces the old one, and entries
carry an "age" so stale ones from earlier searches lose. Mate scores are stored relative to the
position, not the root, then converted back (`adjust_score_for_storage` / `_retrieval`).

### Move ordering

`src/search/move_ordering.rs` sorts moves with this priority:

1. the **hash move**
2. **good captures**, ordered by *most valuable victim, least valuable attacker* (MVV-LVA), where
   a **static exchange evaluation (SEE)** decides whether the capture actually wins material
3. **killer moves**: two quiet moves per ply that recently caused a cutoff in a sibling position
4. the **counter move**: the reply that worked against the opponent's last move
5. other quiet moves, by **history score**: a table `[colour][from][to]` that grows when a move
   causes a cutoff (bonus `depth * depth`, with a "gravity" rule so scores stay below 16,384)
6. **bad captures** (SEE says they lose material) last

### Null-move pruning

Idea: if I am so far ahead that even *passing* leaves me above beta, a real move is certainly
good enough, so give the opponent a free move and search shallower; if that still beats beta,
cut. Conditions in the code: not a PV node, not in check, depth ≥ 3, static eval ≥ beta, and the
side to move has a piece other than pawns (otherwise passing can be the *worst* move, called
zugzwang). Reduction: `R = 2 + depth / 6`.

### Late move reductions (LMR)

With good ordering, the 5th move at a node is rarely the best. So later quiet moves are searched
at *reduced* depth first; only if one surprises you (beats alpha) is it searched again at full
depth. The reduction grows with depth and move number: `ln(depth) * ln(move_number) / 2`,
precomputed in a table. Moves that capture, promote, give check, or are searched while in check
are never reduced.

### Futility and reverse futility pruning

- **Futility**: near the leaves, if the static eval plus a margin (150 cp per ply) is still below
  alpha, quiet moves cannot realistically help, so skip them.
- **Reverse futility**: if the static eval minus a margin (100 cp per ply, up to depth 6) is
  already at or above beta, return it without searching.

### Internal iterative reduction (IIR)

At depth ≥ 4, a node with no hash move has poor ordering, so search it one ply shallower; the
result will leave a hash move behind for later.

### Draw detection

`negamax` returns 0 for: a position that repeats one already on the current line (or a
third occurrence counting the game's history), the 50-move rule (`halfmove_clock >= 100`; checkmate
on that very move still wins) and dead material such as king and bishop against king. The path of
position hashes is kept in `SearchEngine::path`; it is seeded from the game history at the root.
Before this existed (PR #59 fixed it) the engine did not know when it could draw, and scored
king-and-bishop against king as nearly a win.

### What is still missing (PR #62 and the checklist)

On `main`, every move that LMR does not reduce is searched with the full window. **Principal
variation search** (PR #62) searches only the first move that way and checks the rest with a null
window, which is much cheaper and turned out to be worth **+102 Elo**. Read that diff as a
worked example: it is about 20 lines.

**Try it:** switch one technique off at a time (for example set `NULL_MOVE_MIN_DEPTH` to 99) and
see how time to depth 12 and node count change. Then read part 10 to learn why that does not tell
you whether it is *stronger*.

---

## 6. Quiescence search

**File:** `src/search/mod.rs`, function `quiescence`.

If `negamax` stops at depth 0 in the middle of a capture exchange, the evaluation is garbage: you
might evaluate right after taking a queen, before the recapture. This is the **horizon effect**.

So at depth 0 the engine switches to **quiescence search**, which searches only *tactical* moves
(captures and promotions) until the position is quiet:

1. **Stand pat**: the side to move may decline to capture, so the static eval is a lower bound.
   If it already beats beta, return.
2. Try captures, best first. Skip hopeless ones: **delta pruning** (even winning the victim cannot
   reach alpha) and captures that SEE says lose material.
3. If the side to move is **in check**, it cannot stand pat; every legal move is searched.

In a typical search, most nodes are quiescence nodes: in a sample of three positions to depth 8,
about 79%. That is why capture-only move generation and a cheap evaluation matter so much for
speed. The "nodes" the UCI output reports do not include them.

**Try it:** read `generate_legal_tacticals_into` in `src/board/moves.rs` and notice it builds all
pseudo-legal moves and then throws the quiet ones away. A generator that produced only captures
would be faster; that is one of the open speed ideas.

---

## 7. Evaluation

**Files:** `src/eval/`. The entry point is `Evaluator::evaluate`.

The score (in centipawns, 100 ≈ one pawn) is a sum of terms:

| Term | File | Idea |
|---|---|---|
| material and piece-square tables | `pst.rs` | each piece is worth a value, plus a bonus or penalty for the square it stands on (knights like the centre, kings like the corner early) |
| pawn structure | `pawn_structure.rs` | penalties for doubled, isolated and backward pawns; bonuses for passed and connected pawns |
| king safety | `king_safety.rs` | pawns shielding the king, and open files next to it |
| mobility | `mobility.rs` | how many squares each knight, bishop, rook and queen can reach |

The piece-square tables and material values come from the well-known PeSTO evaluation
(itself a Texel-tuned set); the tuner (part 8) can refine the tables and fit the other weights.

### Tapered evaluation

A king wants to hide in the opening and march to the centre in the endgame. So every term has two
versions, **middlegame** and **endgame**, and the score blends them by the **game phase**:

```text
phase = sum of piece weights on the board       (knight 1, bishop 1, rook 2, queen 4; max 24)
score = (mg * phase + eg * (24 - phase)) / 24
```

The phase is 24 with everything on the board and 0 with only kings and pawns.

### Draws in the evaluation

Some material can never win. `Evaluator::evaluate` returns 0 for king against king, king and one
minor piece against king, same-coloured bishops, and king and two knights against king, and for
pawnless opposite-coloured bishops. Without this the tables happily rated king and bishop against
king at +390.

**Try it:** read `EvalWeights` (`src/eval/weights.rs`), pick the doubled pawn penalty, change it,
and run the unit tests that compare known evaluations. Think about how you
would know whether your new value is *better*. That question is part 8.

---

## 8. Tuning the evaluation

**Files:** `src/bin/tuner/`.

You could guess the weights, or you could ask: which weights make the evaluation best predict who
wins? That is **Texel tuning**:

1. Take many quiet positions labelled with the game result (1, 0.5 or 0).
2. Turn the evaluation into a win probability with a sigmoid: `p = 1 / (1 + 10^(-K * eval / 400))`.
3. Compute the average squared error between `p` and the actual result over all positions.
4. Adjust the weights to make that error smaller.

The tuner does exactly this. `fit_k` first finds the best scaling constant `K`, then
`coordinate_descent` tries +1 and -1 on each weight in turn, keeping a change if the error drops,
and repeats until a full sweep improves nothing. It prints new constants ready to paste into the
source. Everything is on CPU threads, since the evaluation is cheap.

**Why it matters for what comes next:** a *neural-net* evaluation (NNUE) is the same idea with
tens of thousands of weights instead of a few hundred. The data pipeline (labelled positions) and
the training loop are the big parts; running the net is a small piece of code.

---

## 9. UCI, threads and time

**Files:** `src/uci/handler.rs`, `src/uci/protocol.rs`, `src/search/time_manager.rs`,
`src/search/smp.rs`.

### The protocol

UCI is plain text over stdin and stdout. A GUI sends `position ... moves ...` and `go ...`; the
engine answers with `info ...` lines and finally `bestmove e2e4`. The engine does not decide
anything about the game; it only answers the question it is asked.

### Why the search runs on a worker thread

If the main thread ran the search, it could not hear `stop` until it finished. So `handle_go`
starts the search on its own thread and the main loop keeps reading input. A shared flag
(`Arc<AtomicBool>`) tells the search to stop; the search checks it every 2,048 nodes.

### Time management

For `go wtime ... btime ...` the engine budgets this move as:

```text
base = remaining / 30 + remaining * 0.05   (+ 0.9 * increment)
hard limit = min(base, 25% of remaining)     // never exceed
soft limit = 60% of the hard limit           // don't start a new depth after this
```

The soft limit is checked between iterations; the hard limit stops a search in progress.

### Lazy SMP (several threads)

With `Threads > 1`, each worker runs the same iterative-deepening search from a different starting
depth (worker *i* starts at depth *1 + i*) and they all share one transposition table. They are
not coordinated; they help each other simply by filling the table. The result comes from the
worker that finished the deepest iteration. It is easy to build and surprisingly effective.

---

## 10. Measuring: how you know a change is good

This is the part most hobby engines get wrong, and where this project is strong. There are four
different questions, and four different tools.

### Is the move generator correct? Perft

Exact. Count leaves, compare with known numbers or with Stockfish. Any mismatch is a bug.

### Is a change faster? `bench/`

`bench/bench.py` builds every commit in a clean container, times a fixed search suite on one pinned
core, and reports **time to depth 12**, **nodes per second**, and **nodes to depth**.

- *Time to depth* is the number that matters for a pure speed-up.
- If *nodes to depth* is **identical** before and after, the search tree is the same, so the
  change is purely about speed. If it differs, the change altered what the engine does, so only
  a match can tell you if it is better.
- `bench.py compare A B` runs the two interleaved and gives a confidence interval.

### Is a change stronger? SPRT matches

Playing games is the only honest test of strength, and it is noisy: one game says almost
nothing. The **sequential probability ratio test (SPRT)** plays games until the evidence is
strong enough, then stops.

You state two hypotheses, for example H0: "the change is worth 0 Elo" and H1: "it is worth +5 Elo",
and two error rates (here 5% each). After every game the test updates a **log-likelihood ratio
(LLR)**: positive evidence pushes it up, negative pushes it down. When it crosses about +2.94 the
test accepts H1; below -2.94 it accepts H0.

What the numbers in this project mean in practice:

| Result | What happened |
|---|---|
| PVS: +102 Elo, 504 games | a huge effect is obvious fast |
| legality speed-up: +54 Elo, 923 games | clear |
| quiescence TT: +31 Elo, 1,715 games | solid, but needs more games |
| a change worth about 0 Elo | never crosses either line, so it runs to the 4,000-game cap and reports "inconclusive" |

That last row is why eight experiments in the catalog are inconclusive: a constant tweak worth a
couple of Elo cannot be told from luck at this budget. The lesson is to spend your effort on
ideas with large effects first.

### How strong is it, absolutely? Elo against fixed opponents

An SPRT only says A beats B. To say "about 2000" you need opponents of known strength. The Elo
tool in `bench/elo.py` (PR #70) plays each version against Stockfish limited to fixed strengths
and fits a rating. The scale is Stockfish's own `UCI_Elo`, which is not the same as CCRL or FIDE
ratings. Read the numbers against each other.

### The workflow for every change

```text
write it -> unit tests + perft -> bench compare (is the tree the same? how much faster?)
         -> SPRT against the version before it -> only then open a pull request
```

---

## 11. Traps this project has already fallen into

Real bugs, so you recognise the shape next time.

| Bug | Lesson |
|---|---|
| No draw detection at all (PR #59) | An engine can be strong at tactics and still not know the rules. Test the rules themselves. |
| The reported PV sometimes ended in an illegal move (issue #57) | A per-ply array was only reset once per iteration, so a node that returned early left a stale length behind. Reset state at the top of the function. |
| `go depth 0` crashed (issue #40) | `clamp(1, max)` panics when `max < 1`. Validate input at the boundary. |
| A tiny clock gave `bestmove 0000` (issue #42) | If the search can stop before it has produced a move, always have a fallback answer. |
| Magic finder took minutes and panicked (issue #7) | It switched to a candidate generator that almost never works and never switched back. A "fallback" can be the bug. |
| The first benchmark run measured the wrong binary | Cargo thought an older commit's files were unchanged because their timestamps were older than the previous build. Build from a clean state when measuring history. |
| A "record" of 0.1% faster | Noise looked like progress. Set a floor below which differences do not count. |

---

## 12. Exercises

Do them in order. For each, write the code yourself; ask for a review when it works.

**Warm-up**

1. **Print a bitboard.** Write `fn print(bb: Bitboard)` that prints an 8x8 grid with rank 8 at the
   top. Check it against the square diagram in part 2 for a few squares.
2. **Count by hand.** From the starting position count the legal moves (20), then for each of
   White's first moves the replies (400 total). Then confirm with `perft` in `tests/perft.rs`.

**The core loop**

3. **Write minimax, then alpha-beta.** On top of `Board::generate_legal_moves` and a
   material-only evaluation, write plain negamax to depth 4 from the starting position and count
   the nodes. Add alpha-beta. The score must not change; the node count must.
4. **Move ordering.** Order captures first and count again. Then try the opposite (captures
   last). The size of the difference is the lesson of part 4.
5. **Horizon effect.** Add a one-ply search of a position where a queen can be captured and
   recaptured. See the wrong answer, then add quiescence and watch it fix itself.

**Pruning**

6. **Break it on purpose.** Disable null-move pruning (`NULL_MOVE_MIN_DEPTH = 99`) and use
   `bench.py compare` to see nodes and time change. Before you run it, write down your
   prediction.
7. **Implement principal variation search** in a copy of `negamax`. Compare your version with the
   one in PR #62: the node counts should be in the same range, and the best move should agree on
   most positions. If they are far apart, find out why before trusting either.
8. **Late move pruning** (a technique `main` does not have): at shallow depth, skip quiet
   moves after the first N. Choose N yourself, then test with an SPRT.

**Measuring**

9. **Read an SPRT.** Pick a finished match from `research/CATALOG.md`, work out what its LLR must
   have done as games accumulated, and explain why it ended where it did.
10. **Predict, then measure.** Pick any change from part 5. Predict its effect on nodes, speed and
    Elo, with numbers. Then measure all three. Keep a list of how wrong you were; that list is
    your education.

**Stretch**

11. Write a captures-only move generator for quiescence and verify the move *set* against
    `generate_legal_tacticals_into` on 1,000 positions.
12. Read the tuner and change the data it trains on. What happens to the weights?

---

## 13. Reading list and glossary

### Where to read more

The [Chess Programming Wiki](https://www.chessprogramming.org) has a page for almost every term
here.

- Board: [Bitboards](https://www.chessprogramming.org/Bitboards),
  [Magic Bitboards](https://www.chessprogramming.org/Magic_Bitboards),
  [Zobrist Hashing](https://www.chessprogramming.org/Zobrist_Hashing),
  [Perft](https://www.chessprogramming.org/Perft), [Pin](https://www.chessprogramming.org/Pin)
- Search: [Negamax](https://www.chessprogramming.org/Negamax),
  [Alpha-Beta](https://www.chessprogramming.org/Alpha-Beta),
  [Iterative Deepening](https://www.chessprogramming.org/Iterative_Deepening),
  [Aspiration Windows](https://www.chessprogramming.org/Aspiration_Windows),
  [Principal Variation Search](https://www.chessprogramming.org/Principal_Variation_Search),
  [Quiescence Search](https://www.chessprogramming.org/Quiescence_Search)
- Speed-ups: [Transposition Table](https://www.chessprogramming.org/Transposition_Table),
  [Move Ordering](https://www.chessprogramming.org/Move_Ordering),
  [MVV-LVA](https://www.chessprogramming.org/MVV-LVA),
  [Static Exchange Evaluation](https://www.chessprogramming.org/Static_Exchange_Evaluation),
  [Killer Heuristic](https://www.chessprogramming.org/Killer_Heuristic),
  [History Heuristic](https://www.chessprogramming.org/History_Heuristic),
  [Countermove Heuristic](https://www.chessprogramming.org/Countermove_Heuristic),
  [Null Move Pruning](https://www.chessprogramming.org/Null_Move_Pruning),
  [Late Move Reductions](https://www.chessprogramming.org/Late_Move_Reductions),
  [Futility Pruning](https://www.chessprogramming.org/Futility_Pruning),
  [Reverse Futility Pruning](https://www.chessprogramming.org/Reverse_Futility_Pruning),
  [Internal Iterative Reductions](https://www.chessprogramming.org/Internal_Iterative_Reductions),
  [Repetitions](https://www.chessprogramming.org/Repetitions),
  [Fifty-move Rule](https://www.chessprogramming.org/Fifty-move_Rule)
- Evaluation: [Tapered Eval](https://www.chessprogramming.org/Tapered_Eval),
  [PeSTO's Evaluation Function](https://www.chessprogramming.org/PeSTO%27s_Evaluation_Function),
  [Pawn Structure](https://www.chessprogramming.org/Pawn_Structure),
  [King Safety](https://www.chessprogramming.org/King_Safety),
  [Mobility](https://www.chessprogramming.org/Mobility),
  [Texel's Tuning Method](https://www.chessprogramming.org/Texel%27s_Tuning_Method),
  [NNUE](https://www.chessprogramming.org/NNUE)
- Engines in practice: [UCI](https://www.chessprogramming.org/UCI),
  [Time Management](https://www.chessprogramming.org/Time_Management),
  [Lazy SMP](https://www.chessprogramming.org/Lazy_SMP),
  [Sequential Probability Ratio Test](https://www.chessprogramming.org/Sequential_Probability_Ratio_Test)

### Glossary

| Term | Meaning |
|---|---|
| **ply** | one move by one side (half a "move") |
| **centipawn (cp)** | a hundredth of a pawn; the unit of evaluation |
| **PV** | principal variation: the best line of play the search found |
| **PV node / non-PV node** | a node searched with a wide window (on the best line) or a null window (everything else) |
| **window** | the (alpha, beta) interval a search is asked about |
| **fail high / fail low** | the score came out at or above beta / at or below alpha |
| **beta cutoff** | stopping the search of a node because a move reached beta |
| **static eval** | the evaluation function's score, with no search |
| **hash move** | the best move stored in the transposition table for this position |
| **node** | one position visited by the search |
| **SEE** | static exchange evaluation: the material outcome of a series of captures on one square |
| **zugzwang** | a position where every move makes things worse, so passing would be best |
| **horizon effect** | misjudging a position because the search stopped mid-exchange |
| **Elo** | a rating scale: +100 Elo is about a 64% score |
| **SPRT** | sequential probability ratio test: play games until the evidence decides |
| **LLR** | log-likelihood ratio: the running evidence score of an SPRT |
| **perft** | counting every leaf of the move tree; the move generator's correctness test |
| **NNUE** | a small neural-network evaluation that updates incrementally as moves are made |
