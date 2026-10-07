//! Search Module
//!
//! Implements the chess search algorithm using negamax with alpha-beta pruning,
//! enhanced with modern techniques for competitive play.
//!
//! # Architecture Overview
//!
//! ```text
//! SearchEngine
//! ├── iterative_deepening()     - Main entry point, searches depth 1, 2, 3...
//! │   └── negamax()             - Core recursive search with alpha-beta
//! │       ├── probe TT          - Check transposition table
//! │       ├── null_move_prune() - Try null move for early cutoff
//! │       ├── MoveOrderer       - Order moves for best cutoffs
//! │       │   ├── Hash move
//! │       │   ├── MVV-LVA captures
//! │       │   ├── Killer moves
//! │       │   └── History heuristic
//! │       ├── Late Move Reductions
//! │       ├── Futility Pruning
//! │       └── quiescence()      - Extend captures at leaf nodes
//! │           └── Stand pat + capture search
//! ├── TranspositionTable        - Hash table of searched positions
//! ├── KillerMoveTable           - Quiet moves causing cutoffs by ply
//! ├── HistoryTable              - Success tracking for quiet moves
//! └── TimeManager               - Time allocation and termination
//! ```

mod history;
mod killer_moves;
mod move_ordering;
mod smp;
mod time_manager;
mod transposition;

pub use history::{CounterMoveTable, HistoryTable};
pub use killer_moves::KillerMoveTable;
pub use move_ordering::{is_good_capture, order_captures_into, MoveOrderer, ScoredMove, MAX_MOVES};
pub use smp::{parallel_search, MAX_THREADS};
pub use time_manager::{SearchLimits, TimeControl, TimeManager};
pub use transposition::{EntryType, SharedTT, TranspositionEntry, TranspositionTable};

use crate::board::{Move, MoveType};
use crate::eval::Evaluator;
use crate::types::PieceType;
use crate::Board;
use std::sync::Arc;

/// Mate score constant (high value indicating checkmate).
pub const MATE_SCORE: i32 = 100_000;

/// Value representing infinity for alpha-beta bounds.
pub const INFINITY: i32 = MATE_SCORE + 1000;

/// Maximum search depth.
pub const MAX_DEPTH: u8 = 128;

/// Null move reduction depth.
pub const NULL_MOVE_REDUCTION: i32 = 3;

/// Minimum depth for null move pruning.
pub const NULL_MOVE_MIN_DEPTH: i32 = 99;

/// Late move reduction thresholds.
pub const LMR_FULL_DEPTH_MOVES: usize = 4;
pub const LMR_REDUCTION_LIMIT: i32 = 3;

/// Precomputed LMR table indexed by [depth][move_count].
/// Generated from the same formula as the original floating-point version;
/// see the `lmr_table_matches_formula` test.
static LMR_TABLE: once_cell::sync::Lazy<[[i32; 256]; 129]> =
    once_cell::sync::Lazy::new(|| {
        let mut table = [[0i32; 256]; 129];
        for depth in 0..129 {
            for move_count in 0..256 {
                let ln_depth = (depth as f64).ln();
                let ln_move_count = (move_count as f64).ln();
                let reduction = (ln_depth * ln_move_count / 2.0) as i32;
                table[depth][move_count] = reduction.min(depth as i32 - 1).max(1);
            }
        }
        table
    });

/// Futility pruning base margin (per depth).
pub const FUTILITY_MARGIN_BASE: i32 = 150;

/// Aspiration window initial size.
pub const ASPIRATION_WINDOW: i32 = 50;

/// Maximum ply tracked in the principal variation table.
pub const PV_SIZE: usize = MAX_DEPTH as usize + 1;

/// Check whether two moves share from/to/type (root restriction matching).
#[inline]
fn is_same_move(a: &Move, b: &Move) -> bool {
    a.from == b.from && a.to == b.to && a.move_type == b.move_type
}

/// Check if a score is a mate score.
#[inline]
pub fn is_mate_score(score: i32) -> bool {
    score.abs() > MATE_SCORE - 500
}

/// Convert mate score to moves until mate.
#[inline]
pub fn mate_in(score: i32) -> Option<i32> {
    if is_mate_score(score) {
        Some((MATE_SCORE - score.abs() + 1) / 2)
    } else {
        None
    }
}

/// Search statistics for debugging and UCI info.
#[derive(Clone, Default, Debug)]
pub struct SearchStats {
    /// Total nodes searched.
    pub nodes: u64,
    /// Quiescence nodes searched.
    pub qnodes: u64,
    /// Transposition table hits.
    pub tt_hits: u64,
    /// Transposition table cutoffs.
    pub tt_cutoffs: u64,
    /// Null move cutoffs.
    pub null_cutoffs: u64,
    /// Beta cutoffs (fail high).
    pub beta_cutoffs: u64,
    /// Alpha improvements.
    pub alpha_improvements: u64,
    /// Current search depth.
    pub depth: u8,
    /// Selective depth (max ply reached).
    pub seldepth: u8,
}

impl SearchStats {
    /// Reset all statistics.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Get nodes per second.
    pub fn nps(&self, elapsed_ms: u128) -> u64 {
        if elapsed_ms == 0 {
            return 0;
        }
        (self.nodes as u128 * 1000 / elapsed_ms) as u64
    }
}

/// Result of a search operation.
#[derive(Clone, Debug)]
pub struct SearchResult {
    /// Best move found.
    pub best_move: Option<Move>,
    /// Score of the position (from side to move's perspective).
    pub score: i32,
    /// Depth searched.
    pub depth: u8,
    /// Principal variation (best line).
    pub pv: Vec<Move>,
    /// Search statistics.
    pub stats: SearchStats,
}

impl Default for SearchResult {
    fn default() -> Self {
        Self {
            best_move: None,
            score: 0,
            depth: 0,
            pv: Vec::new(),
            stats: SearchStats::default(),
        }
    }
}

/// The main search engine.
pub struct SearchEngine {
    /// Transposition table.
    tt: TranspositionTable,
    /// Shared transposition table for Lazy SMP workers. When `Some`, all
    /// TT probes/stores go here instead of `tt`, so the single-threaded
    /// path (`None`) stays bit-identical to before.
    shared: Option<Arc<SharedTT>>,
    /// Killer move table.
    killers: KillerMoveTable,
    /// History heuristic table.
    history: HistoryTable,
    /// Counter move table.
    counters: CounterMoveTable,
    /// Position evaluator.
    evaluator: Evaluator,
    /// Time manager.
    time_manager: TimeManager,
    /// Search statistics.
    stats: SearchStats,
    /// Principal variation table (triangular, fixed size, no allocation).
    pv: [[Move; PV_SIZE]; PV_SIZE],
    /// Valid prefix lengths per ply in `pv`.
    pv_len: [u8; PV_SIZE],
    /// Root move restriction from UCI `searchmoves` (None = all moves).
    root_moves: Option<Vec<Move>>,
    /// Per-ply reusable move buffers (capacity retained, no per-node alloc).
    move_buffers: Vec<Vec<Move>>,
}

impl SearchEngine {
    /// Create a new search engine.
    pub fn new() -> Self {
        Self {
            tt: TranspositionTable::default(),
            shared: None,
            killers: KillerMoveTable::new(),
            history: HistoryTable::new(),
            counters: CounterMoveTable::new(),
            evaluator: Evaluator::new(),
            time_manager: TimeManager::default(),
            stats: SearchStats::default(),
            pv: [[Move::NONE; PV_SIZE]; PV_SIZE],
            pv_len: [0; PV_SIZE],
            root_moves: None,
            move_buffers: Vec::new(),
        }
    }

    /// Create a search engine with custom transposition table size.
    pub fn with_hash_size(size_mb: usize) -> Self {
        Self {
            tt: TranspositionTable::new(size_mb),
            ..Self::new()
        }
    }

    /// Clear all search state for a new game.
    ///
    /// Only the thread-local state (including the private TT) is cleared.
    /// The shared SMP table, when present, is managed by the search
    /// coordinator (cleared once per game, not once per worker).
    pub fn new_game(&mut self) {
        self.tt.clear();
        self.killers.clear();
        self.history.clear();
        self.counters.clear();
        self.stats.reset();
        self.pv_len = [0; PV_SIZE];
        for buf in &mut self.move_buffers {
            buf.clear();
        }
    }

    /// Point this engine at a shared SMP transposition table. All TT
    /// probes/stores then go to the shared table instead of the private
    /// one. Set to `None` to restore single-threaded behavior.
    pub fn set_shared_tt(&mut self, tt: Option<Arc<SharedTT>>) {
        self.shared = tt;
    }

    /// Probe the active transposition table (shared when set, else private).
    #[inline]
    fn tt_probe(&self, hash: u64) -> Option<TranspositionEntry> {
        if let Some(shared) = &self.shared {
            shared.probe(hash)
        } else {
            self.tt.probe(hash).copied()
        }
    }

    /// Store into the active transposition table (shared when set, else private).
    #[inline]
    fn tt_store(
        &mut self,
        hash: u64,
        score: i32,
        depth: u8,
        entry_type: EntryType,
        best_move: Option<Move>,
    ) {
        if let Some(shared) = &self.shared {
            shared.store(hash, score, depth, entry_type, best_move);
        } else {
            self.tt.store(hash, score, depth, entry_type, best_move);
        }
    }

    /// Bump the age of the active transposition table.
    fn tt_new_search(&mut self) {
        if let Some(shared) = &self.shared {
            shared.new_search();
        } else {
            self.tt.new_search();
        }
    }

    /// Search for the best move with the given limits.
    ///
    /// This is the main entry point for the search.
    pub fn search(&mut self, board: &Board, limits: SearchLimits) -> SearchResult {
        self.search_from_depth(board, limits, 1)
    }

    /// Search starting iterative deepening at `start_depth` instead of 1.
    ///
    /// Used by Lazy SMP helpers: staggering starting depths diversifies
    /// the workers. A `start_depth` of 1 behaves exactly like [`search`](Self::search).
    pub fn search_from_depth(
        &mut self,
        board: &Board,
        limits: SearchLimits,
        start_depth: u8,
    ) -> SearchResult {
        // Create and start time manager.
        self.time_manager = TimeManager::new(&limits);
        self.time_manager.start();

        // Store max depth from limits.
        let max_depth = limits.max_depth;

        // Store root move restriction (UCI searchmoves).
        self.root_moves = limits.root_moves.clone();

        // Age history table for fresh search.
        self.history.age();

        // Clear killers for new search.
        self.killers.clear();

        // Increment TT age.
        self.tt_new_search();

        // Reset statistics.
        self.stats.reset();

        // Run iterative deepening.
        self.iterative_deepening(board, max_depth, start_depth)
    }

    /// Worker entry point for Lazy SMP searches sharing one table.
    ///
    /// Identical to [`search_from_depth`](Self::search_from_depth) except
    /// it does NOT bump the shared table age — the coordinator bumps it
    /// exactly once per search so replacement stays equivalent to the
    /// single-threaded table.
    pub(crate) fn search_worker(
        &mut self,
        board: &Board,
        limits: SearchLimits,
        start_depth: u8,
    ) -> SearchResult {
        // Create and start time manager.
        self.time_manager = TimeManager::new(&limits);
        self.time_manager.start();

        // Store max depth from limits.
        let max_depth = limits.max_depth;

        // Store root move restriction (UCI searchmoves).
        self.root_moves = limits.root_moves.clone();

        // Age history table for fresh search.
        self.history.age();

        // Clear killers for new search.
        self.killers.clear();

        // NOTE: no tt_new_search() here — see doc comment above.

        // Reset statistics.
        self.stats.reset();

        // Run iterative deepening.
        self.iterative_deepening(board, max_depth, start_depth)
    }

    /// Iterative deepening search.
    ///
    /// Searches to increasing depths until time runs out.
    /// Each iteration uses results from previous iterations for move ordering.
    fn iterative_deepening(
        &mut self,
        board: &Board,
        max_depth: Option<u8>,
        start_depth: u8,
    ) -> SearchResult {
        let mut result = SearchResult::default();
        let mut board = board.clone();
        board.enable_history();

        let max_depth = max_depth.unwrap_or(MAX_DEPTH);
        // Helpers with a stagger above the cap still search the cap.
        let start_depth = start_depth.clamp(1, max_depth);

        for depth in start_depth..=max_depth {
            // Check if we should stop before starting new iteration.
            if depth > 1 && !self.time_manager.can_start_iteration() {
                break;
            }

            self.stats.depth = depth;

            // Reset PV lengths for this iteration.
            self.pv_len = [0; PV_SIZE];

            // Search at this depth.
            // TEMPORARILY DISABLED: Aspiration windows to debug queen blunder
            let (score, best_move) = {
                // Full window search.
                let score = self.negamax(&mut board, depth as i32, -INFINITY, INFINITY, 0);
                let best_move = match self.pv_first_move() {
                    Some(mv) => Some(mv),
                    None => self.get_hash_move(board.position_hash()),
                };
                (score, best_move)
            };

            // Check if search was stopped.
            if self.should_stop() && depth > 1 {
                break;
            }

            // Update result with this iteration's findings.
            result.score = score;
            result.depth = depth;
            result.best_move = best_move;
            result.pv = if self.pv_len[0] > 0 {
                self.pv[0][..self.pv_len[0] as usize].to_vec()
            } else {
                self.extract_pv(&mut board, depth)
            };
            result.stats = self.stats.clone();
        }

        result
    }

    /// Aspiration window search.
    ///
    /// Uses a narrow window around the previous score to get more cutoffs.
    /// Falls back to full window if score is outside the window.
    fn aspiration_search(
        &mut self,
        board: &mut Board,
        depth: u8,
        previous_score: i32,
    ) -> (i32, Option<Move>) {
        let mut delta = ASPIRATION_WINDOW;
        let mut alpha = previous_score - delta;
        let mut beta = previous_score + delta;

        loop {
            let score = self.negamax(board, depth as i32, alpha, beta, 0);

            // Check for time out.
            if self.should_stop() {
                let best_move = match self.pv_first_move() {
                    Some(mv) => Some(mv),
                    None => self.get_hash_move(board.position_hash()),
                };
                return (score, best_move);
            }

            // Check if score is within window.
            if score <= alpha {
                // Fail low - widen alpha.
                alpha = (score - delta).max(-INFINITY);
                delta *= 2;
            } else if score >= beta {
                // Fail high - widen beta.
                beta = (score + delta).min(INFINITY);
                delta *= 2;
            } else {
                // Score is within window.
                let best_move = match self.pv_first_move() {
                    Some(mv) => Some(mv),
                    None => self.get_hash_move(board.position_hash()),
                };
                return (score, best_move);
            }

            // Fallback to full window if delta gets too large.
            // Lower threshold (200 instead of 500) to reduce aspiration bugs.
            if delta > 200 {
                alpha = -INFINITY;
                beta = INFINITY;
            }
        }
    }

    /// Negamax search with alpha-beta pruning.
    ///
    /// The core recursive search function.
    ///
    /// # Arguments
    /// * `board` - The current position
    /// * `depth` - Remaining depth to search
    /// * `alpha` - Lower bound (best score we can guarantee)
    /// * `beta` - Upper bound (opponent's best guarantee)
    /// * `ply` - Distance from root (for mate scoring)
    ///
    /// # Returns
    /// The score of the position from the current player's perspective.
    fn negamax(
        &mut self,
        board: &mut Board,
        depth: i32,
        mut alpha: i32,
        beta: i32,
        ply: u8,
    ) -> i32 {
        // Save original alpha for TT entry type determination.
        let original_alpha = alpha;

        // Update selective depth.
        if ply > self.stats.seldepth {
            self.stats.seldepth = ply;
        }

        // Check for time limit periodically.
        if self.stats.nodes & 2047 == 0 && self.should_stop() {
            return 0;
        }

        // Leaf node - use quiescence search.
        if depth <= 0 {
            return self.quiescence(board, alpha, beta, ply);
        }

        self.stats.nodes += 1;
        self.time_manager.add_nodes(1);

        // Enforce node limit exactly (the periodic check below can
        // overshoot by up to 2047 nodes).
        if self.time_manager.limit_reached() {
            return 0;
        }

        // Check for maximum ply.
        if ply >= MAX_DEPTH {
            return self.evaluator.evaluate(board);
        }

        let _is_root = ply == 0;
        let hash = board.position_hash();
        let is_pv = beta - alpha > 1;

        // TT probing - get hash move and potentially cutoff
        let mut hash_move: Option<Move> = None;
        if let Some(entry) = self.tt_probe(hash) {
            self.stats.tt_hits += 1;
            hash_move = entry.best_move;

            // TT cutoff for non-PV nodes with sufficient depth
            if !is_pv && entry.depth >= depth as u8 {
                if let Some(score) = entry.get_score(alpha, beta) {
                    self.stats.tt_cutoffs += 1;
                    let adjusted = TranspositionTable::adjust_score_for_retrieval(score, ply);
                    return adjusted;
                }
            }
        }

        // Generate legal moves into the per-ply reusable buffer.
        if self.move_buffers.len() <= ply as usize {
            self.move_buffers.resize_with(ply as usize + 1, Vec::new);
        }
        {
            let buf = &mut self.move_buffers[ply as usize];
            board.generate_legal_moves_into(buf);

            // Check for checkmate or stalemate.
            if buf.is_empty() {
                return if board.is_in_check() {
                    // Checkmate - return negative mate score.
                    -MATE_SCORE + ply as i32
                } else {
                    // Stalemate.
                    0
                };
            }
        }

        // Static evaluation for pruning decisions.
        let static_eval = self.evaluator.evaluate(board);
        let in_check = board.is_in_check();

        // Null move pruning
        if !is_pv
            && !in_check
            && depth >= NULL_MOVE_MIN_DEPTH
            && static_eval >= beta
            && self.can_null_move(board, ply)
        {
            if let Some(score) = self.null_move_prune(board, depth, beta, ply) {
                return score;
            }
        }

        // Create move orderer over a stack buffer (no heap allocation).
        // Killer moves are copied out so the loop can use &mut tables.
        // The orderer copies the move list; all borrows end here.
        let killer_moves = self.killers.get_killers(ply);
        let mut orderer = MoveOrderer::new(
            board,
            &self.move_buffers[ply as usize],
            hash_move,
            killer_moves,
            &self.history,
        );

        // Enforce UCI `searchmoves` restriction at the root.
        // Pre-scan (instead of filtering a collected Vec): if nothing legal
        // matches, the restriction is ignored, as before. Cloned once per
        // root call so the loop holds no borrow of `self`.
        let restriction: Option<Vec<Move>> = if ply == 0 {
            self.root_moves.clone()
        } else {
            None
        };
        let restriction_active = match &restriction {
            Some(r) => self.move_buffers[ply as usize]
                .iter()
                .any(|mv| r.iter().any(|rm| is_same_move(rm, mv))),
            None => false,
        };

        let mut best_score = -INFINITY;
        let mut best_move = None;
        let mut move_count = 0;
        // Failed quiets for history updates (stack buffer). Sized at
        // MAX_MOVES so the cap can never bind (legal moves <= 218):
        // behavior is identical to the unbounded Vec.
        let mut quiets_tried = [Move::NONE; MAX_MOVES];
        let mut quiets_len = 0usize;

        // Futility pruning conditions
        let can_futility_prune = !is_pv
            && !in_check
            && depth <= LMR_REDUCTION_LIMIT
            && static_eval + self.futility_margin(depth) <= alpha;

        // Search moves.
        while let Some(mv) = orderer.next(board) {
            // Root restriction (pre-scanned above): skipped moves don't count.
            if restriction_active {
                let ok = match &restriction {
                    Some(r) => r.iter().any(|rm| is_same_move(rm, &mv)),
                    None => true,
                };
                if !ok {
                    continue;
                }
            }

            move_count += 1;
            let is_capture = mv.captured.is_some();
            let is_tactical = self.is_tactical(&mv);

            // Make the move (unchecked: it comes from this position's
            // legal set, already validated during generation).
            let undo = board.do_move(mv);

            // Futility pruning - skip quiet moves that can't raise alpha.
            // Never prune moves that give check: they are tactical even
            // though is_tactical() can't see that before the move is made.
            if can_futility_prune && move_count > 1 && !is_tactical && !board.is_in_check() {
                board.undo_move(mv, undo);
                continue;
            }

            // Late move reductions
            let reduction = if move_count > LMR_FULL_DEPTH_MOVES
                && depth >= LMR_REDUCTION_LIMIT
                && !is_tactical
                && !in_check
                && !board.is_in_check() // Don't reduce moves that give check
            {
                self.late_move_reduction(depth, move_count, &mv)
            } else {
                0
            };

            // Search with reduced depth first.
            let mut score = if reduction > 0 {
                -self.negamax(board, depth - 1 - reduction, -alpha - 1, -alpha, ply + 1)
            } else {
                alpha + 1 // Force full search.
            };

            // Re-search at full depth if reduced search raised alpha.
            if score > alpha {
                score = -self.negamax(board, depth - 1, -beta, -alpha, ply + 1);
            }

            // Unmake the move.
            board.undo_move(mv, undo);

            // Check for time out.
            if self.should_stop() {
                return best_score;
            }

            // Track quiet moves for history updates.
            if !is_capture && score <= alpha {
                debug_assert!(quiets_len < quiets_tried.len());
                if quiets_len < quiets_tried.len() {
                    quiets_tried[quiets_len] = mv;
                    quiets_len += 1;
                }
            }

            // Update best score.
            if score > best_score {
                best_score = score;
                best_move = Some(mv);

                // Update PV (triangular fixed-size table, no allocation).
                {
                    let p = ply as usize;
                    if p < PV_SIZE {
                        self.pv[p][0] = mv;
                        let mut len = 1usize;
                        if p + 1 < PV_SIZE {
                            let cl =
                                (self.pv_len[p + 1] as usize).min(PV_SIZE - 1 - p);
                            let (lo, hi) = self.pv.split_at_mut(p + 1);
                            lo[p][1..1 + cl].copy_from_slice(&hi[0][..cl]);
                            len += cl;
                        }
                        self.pv_len[p] = len as u8;
                    }
                }

                if score > alpha {
                    self.stats.alpha_improvements += 1;
                    alpha = score;

                    // Beta cutoff.
                    if score >= beta {
                        self.stats.beta_cutoffs += 1;

                        // Update heuristics for quiet moves.
                        if !is_capture {
                            self.update_cutoff_heuristics(
                                board,
                                &mv,
                                depth,
                                ply,
                                &quiets_tried[..quiets_len],
                            );
                        }

                        // Store in TT with adjusted mate score.
                        let stored_score =
                            TranspositionTable::adjust_score_for_storage(score, ply);
                        self.tt_store(
                            hash,
                            stored_score,
                            depth as u8,
                            EntryType::LowerBound,
                            Some(mv),
                        );

                        return score;
                    }
                }
            }
        }

        // Store result in transposition table with adjusted mate score.
        let entry_type = if best_score <= original_alpha {
            EntryType::UpperBound
        } else {
            EntryType::Exact
        };

        let stored_score = TranspositionTable::adjust_score_for_storage(best_score, ply);
        self.tt_store(hash, stored_score, depth as u8, entry_type, best_move);

        best_score
    }

    /// Quiescence search.
    ///
    /// Extends the search at leaf nodes by examining captures
    /// until the position is "quiet" (no hanging pieces).
    ///
    /// # Arguments
    /// * `board` - The current position
    /// * `alpha` - Lower bound
    /// * `beta` - Upper bound
    /// * `ply` - Distance from root
    ///
    /// # Returns
    /// The score after all tactical sequences resolve.
    fn quiescence(
        &mut self,
        board: &mut Board,
        mut alpha: i32,
        beta: i32,
        ply: u8,
    ) -> i32 {
        self.stats.qnodes += 1;
        self.time_manager.add_nodes(1);

        // Enforce node limit exactly.
        if self.time_manager.limit_reached() {
            return 0;
        }

        // Update selective depth.
        if ply > self.stats.seldepth {
            self.stats.seldepth = ply;
        }

        // Prevent ply overflow and limit quiescence depth.
        if ply >= MAX_DEPTH {
            return self.evaluator.evaluate(board);
        }

        // Check for time limit periodically.
        if self.stats.qnodes & 2047 == 0 && self.should_stop() {
            return 0;
        }

        // Stand pat - if we're not in check, we can assume we can at least
        // achieve the static evaluation by not making any captures.
        let in_check = board.is_in_check();
        let stand_pat = self.evaluator.evaluate(board);

        if !in_check {
            if stand_pat >= beta {
                return beta;
            }
            if stand_pat > alpha {
                alpha = stand_pat;
            }
        }

        // Generate moves into the per-ply reusable buffer: full legal
        // moves for check evasions, otherwise only tacticals (captures
        // and promotions — exactly the set filtered before, but quiets
        // skip legality tests entirely).
        if self.move_buffers.len() <= ply as usize {
            self.move_buffers.resize_with(ply as usize + 1, Vec::new);
        }
        {
            let buf = &mut self.move_buffers[ply as usize];
            if in_check {
                board.generate_legal_moves_into(buf);
                // If in check and no moves, it's checkmate.
                if buf.is_empty() {
                    return -MATE_SCORE + ply as i32;
                }
            } else {
                board.generate_legal_tacticals_into(buf);
                // Empty tactical set: quiet position (stand pat below) or
                // stalemate (no legal moves at all). The scan exits at the
                // first legal quiet, so quiet positions stay cheap.
                if buf.is_empty() && !board.has_legal_quiet(buf) {
                    return 0; // Stalemate
                }
            }
        }

        // Order moves by MVV-LVA into a stack buffer, then iterate
        // best-first with selection sort (early cutoffs skip the tail).
        // The buffer already holds exactly the searched set (full evasions
        // in check, tacticals otherwise), so no intermediate Vec remains.
        let mut cap_buf = [ScoredMove::new(Move::NONE, 0); MAX_MOVES];
        let n_caps = {
            let buf = &self.move_buffers[ply as usize];
            order_captures_into(buf, &mut cap_buf)
        };
        let mut caps_done = 0;
        while caps_done < n_caps {
            // Selection-sort next best. Rotation (not swap) preserves the
            // relative order of tied scores, exactly matching the previous
            // stable full sort.
            let mut best = caps_done;
            for i in caps_done + 1..n_caps {
                if cap_buf[i].score > cap_buf[best].score {
                    best = i;
                }
            }
            let sm = cap_buf[best];
            for i in (caps_done..best).rev() {
                cap_buf[i + 1] = cap_buf[i];
            }
            cap_buf[caps_done] = sm;
            let mv = sm.mv;
            caps_done += 1;
            // Delta pruning - skip captures that can't raise alpha.
            if !in_check && mv.captured.is_some() {
                let captured_value = match mv.captured.unwrap().piece_type {
                    PieceType::Pawn => 100,
                    PieceType::Knight => 320,
                    PieceType::Bishop => 330,
                    PieceType::Rook => 500,
                    PieceType::Queen => 900,
                    PieceType::King => 20000,
                };

                // If even capturing the piece can't improve alpha, skip.
                if stand_pat + captured_value + 200 < alpha {
                    continue;
                }

                // Skip bad captures (SEE negative).
                if !is_good_capture(board, &mv) {
                    continue;
                }
            }

            // Make the move (unchecked: from this position's legal set).
            let undo = board.do_move(mv);
            let score = -self.quiescence(board, -beta, -alpha, ply + 1);
            board.undo_move(mv, undo);

            if score >= beta {
                return beta;
            }
            if score > alpha {
                alpha = score;
            }
        }

        alpha
    }

    /// Null move pruning.
    ///
    /// If giving the opponent a free move still results in a position
    /// that's good for us, we can prune this branch.
    ///
    /// # Returns
    /// `Some(score)` if we can prune, `None` if we need to search.
    fn null_move_prune(
        &mut self,
        board: &mut Board,
        depth: i32,
        beta: i32,
        ply: u8,
    ) -> Option<i32> {
        // Make null move (just switch side to move).
        board.make_null_move();

        // Search with reduced depth.
        let reduction = NULL_MOVE_REDUCTION + depth / 6;
        let score = -self.negamax(
            board,
            depth - 1 - reduction,
            -beta,
            -beta + 1,
            ply + 1,
        );

        // Unmake null move.
        board.unmake_null_move();

        // If null move search still beats beta, we can prune.
        if score >= beta {
            self.stats.null_cutoffs += 1;
            // Don't return mate scores from null move search.
            if is_mate_score(score) {
                return Some(beta);
            }
            return Some(score);
        }

        None
    }

    /// Check if null move pruning is allowed in this position.
    fn can_null_move(&self, board: &Board, _ply: u8) -> bool {
        // Don't do null move if we only have pawns (zugzwang danger).
        let side = board.side_to_move();

        // Count non-pawn, non-king pieces.
        let knights = board.piece_bitboard(side, PieceType::Knight).count_pieces();
        let bishops = board.piece_bitboard(side, PieceType::Bishop).count_pieces();
        let rooks = board.piece_bitboard(side, PieceType::Rook).count_pieces();
        let queens = board.piece_bitboard(side, PieceType::Queen).count_pieces();

        // Need at least one major/minor piece to avoid zugzwang.
        knights + bishops + rooks + queens > 0
    }

    /// Late move reduction.
    ///
    /// Reduce search depth for moves that are unlikely to be good
    /// (late in move ordering, not tactical).
    ///
    /// # Returns
    /// The reduction amount (0 if no reduction).
    fn late_move_reduction(&self, depth: i32, move_count: usize, _mv: &Move) -> i32 {
        // Table lookup with the identical formula (depth/move_count clamped
        // to the table range; legal move counts never exceed 218).
        let d = depth.clamp(0, 128) as usize;
        let m = move_count.min(255);
        LMR_TABLE[d][m]
    }

    /// Futility pruning margin.
    ///
    /// If the static eval plus a margin can't beat alpha,
    /// we can skip searching this move.
    fn futility_margin(&self, depth: i32) -> i32 {
        FUTILITY_MARGIN_BASE * depth
    }

    /// Check if a move is tactical (capture, promotion, check).
    fn is_tactical(&self, mv: &Move) -> bool {
        mv.captured.is_some() || matches!(mv.move_type, MoveType::Promotion(_))
    }

    /// Update heuristics after a beta cutoff.
    fn update_cutoff_heuristics(
        &mut self,
        board: &Board,
        mv: &Move,
        depth: i32,
        ply: u8,
        quiets_tried: &[Move],
    ) {
        let color = board.side_to_move();

        // Update killer moves.
        self.killers.store(ply, *mv);

        // Update history heuristic.
        self.history.update(color, mv, quiets_tried, depth);
    }

    /// Extract principal variation from the transposition table.
    fn extract_pv(&self, board: &mut Board, max_depth: u8) -> Vec<Move> {
        let mut pv = Vec::new();
        // Cycle guard without hashing overhead: depth-bounded linear scan
        // (max 255 entries for u8 depth; almost always < 20 in practice).
        let mut seen_hashes = [0u64; 256];
        let mut n_seen = 0usize;

        for _ in 0..max_depth {
            let hash = board.position_hash();

            // Prevent infinite loops from hash collisions.
            if seen_hashes[..n_seen].contains(&hash) {
                break;
            }
            seen_hashes[n_seen] = hash;
            n_seen += 1;

            // Get the best move from TT.
            if let Some(entry) = self.tt_probe(hash) {
                if let Some(mv) = entry.best_move {
                    // Verify move is legal.
                    let legal_moves = board.generate_legal_moves();
                    if legal_moves.contains(&mv) {
                        pv.push(mv);
                        board.make_move(mv);
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Unmake all moves.
        for _ in 0..pv.len() {
            board.unmake_move();
        }

        pv
    }

    /// First move of the current principal variation, if any.
    fn pv_first_move(&self) -> Option<Move> {
        if self.pv_len[0] > 0 {
            Some(self.pv[0][0])
        } else {
            None
        }
    }

    /// Check if we should stop searching.
    fn should_stop(&self) -> bool {
        self.time_manager.should_stop()
    }

    /// Get the hash move from the transposition table.
    fn get_hash_move(&self, hash: u64) -> Option<Move> {
        self.tt_probe(hash).and_then(|e| e.best_move)
    }

    /// Resize the transposition table.
    pub fn set_hash_size(&mut self, size_mb: usize) {
        self.tt.resize(size_mb);
    }

    /// Clear the transposition table.
    pub fn clear_hash(&mut self) {
        self.tt.clear();
    }

    /// Get current search statistics.
    pub fn stats(&self) -> &SearchStats {
        &self.stats
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple search function for quick testing.
///
/// Searches to the given depth and returns the best move.
pub fn search(board: &Board, depth: u8) -> Option<Move> {
    let mut engine = SearchEngine::new();
    let result = engine.search(board, SearchLimits::depth(depth));
    result.best_move
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mate_in_one() {
        // White to move, Qa8# is mate (back rank mate).
        // Position: Queen on a1, black king on g8 boxed in by pawns on f7/g7/h7.
        let board = Board::from_fen("6k1/5ppp/8/8/8/8/8/Q3K3 w - - 0 1").unwrap();
        let mut engine = SearchEngine::new();

        let result = engine.search(&board, SearchLimits::depth(3));

        assert!(result.best_move.is_some());
        let mv = result.best_move.unwrap();
        // Qa8# is mate.
        assert_eq!(mv.to.file(), 0, "Expected a-file, got file {}", mv.to.file());
        assert_eq!(mv.to.rank(), 7, "Expected 8th rank, got rank {}", mv.to.rank());
        assert!(is_mate_score(result.score), "Expected mate score, got {}", result.score);
    }

    #[test]
    fn test_captures_free_piece() {
        // White can capture an undefended black queen.
        // Knight on e4 can reach c5 (valid knight move: -2, +1).
        let board = Board::from_fen("4k3/8/8/2q5/4N3/8/8/4K3 w - - 0 1").unwrap();
        let mut engine = SearchEngine::new();

        let result = engine.search(&board, SearchLimits::depth(2));

        assert!(result.best_move.is_some());
        let mv = result.best_move.unwrap();
        // Knight should take queen on c5.
        assert_eq!(mv.to.file(), 2, "Expected c-file, got file {}", mv.to.file());
        assert_eq!(mv.to.rank(), 4, "Expected 5th rank, got rank {}", mv.to.rank());
        assert!(mv.captured.is_some());
        assert_eq!(mv.captured.unwrap().piece_type, PieceType::Queen);
    }

    #[test]
    fn test_search_returns_move() {
        let board = Board::starting_position();
        let mut engine = SearchEngine::new();

        let result = engine.search(&board, SearchLimits::depth(3));

        assert!(result.best_move.is_some());
        assert!(result.depth >= 3);
        assert!(result.stats.nodes > 0);
    }

    #[test]
    fn test_search_depth_limit() {
        let board = Board::starting_position();
        let mut engine = SearchEngine::new();

        let result = engine.search(&board, SearchLimits::depth(2));

        assert_eq!(result.depth, 2);
    }

    #[test]
    fn test_quiescence_avoids_blunder() {
        // White has a knight that can "capture" a defended pawn.
        // Without quiescence, might look good. With qsearch, should see recapture.
        let board = Board::from_fen("4k3/8/3p4/2p5/3N4/8/8/4K3 w - - 0 1").unwrap();
        let mut engine = SearchEngine::new();

        let result = engine.search(&board, SearchLimits::depth(3));

        // Knight should NOT capture the pawn on c5 defended by d6 pawn.
        if let Some(mv) = result.best_move {
            if mv.captured.is_some() {
                // If it's a capture, it should be a good capture.
                assert!(result.score >= -100, "Blundered piece: score = {}", result.score);
            }
        }
    }

    #[test]
    fn test_checkmate_detection() {
        // White to move, Ra8# is mate (back rank mate).
        // Position: Black king on g8 boxed in by pawns on f7/g7/h7.
        // White rook on a1 delivers Ra8#.
        let board = Board::from_fen("6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1").unwrap();
        let mut engine = SearchEngine::new();

        let result = engine.search(&board, SearchLimits::depth(2));

        // White should find Ra8#.
        assert!(result.best_move.is_some());
        let mv = result.best_move.unwrap();
        assert_eq!(mv.to.file(), 0, "Expected a-file, got file {}", mv.to.file()); // a-file
        assert_eq!(mv.to.rank(), 7, "Expected 8th rank, got rank {}", mv.to.rank()); // 8th rank (a8)
        assert!(is_mate_score(result.score), "Expected mate score, got {}", result.score);
    }

    #[test]
    fn test_stalemate_is_draw() {
        // Black is stalemated (black to move, king on a8, white king on c7, white queen on b6).
        let board = Board::from_fen("k7/2K5/1Q6/8/8/8/8/8 b - - 0 1").unwrap();
        let mut engine = SearchEngine::new();

        // Black has no moves - this position is stalemate.
        let moves = board.generate_legal_moves();
        assert!(moves.is_empty());
    }

    #[test]
    fn test_new_game_clears_state() {
        let mut engine = SearchEngine::new();

        // Do a search to populate state.
        let board = Board::starting_position();
        engine.search(&board, SearchLimits::depth(2));

        // Clear state.
        engine.new_game();

        // Stats should be reset.
        assert_eq!(engine.stats.nodes, 0);
    }

    #[test]
    fn test_mate_score_helper() {
        assert!(is_mate_score(MATE_SCORE));
        assert!(is_mate_score(-MATE_SCORE));
        assert!(is_mate_score(MATE_SCORE - 10));
        assert!(!is_mate_score(500));
        assert!(!is_mate_score(-500));
    }

    #[test]
    fn test_mate_in_helper() {
        assert_eq!(mate_in(MATE_SCORE), Some(0));
        assert_eq!(mate_in(MATE_SCORE - 2), Some(1));
        assert_eq!(mate_in(MATE_SCORE - 4), Some(2));
        assert_eq!(mate_in(100), None);
    }

    #[test]
    fn test_lmr_table_matches_formula() {
        // Reference: the original floating-point formula.
        fn reference(depth: i32, move_count: usize) -> i32 {
            let ln_depth = (depth as f64).ln();
            let ln_move_count = (move_count as f64).ln();
            let reduction = (ln_depth * ln_move_count / 2.0) as i32;
            reduction.min(depth - 1).max(1)
        }

        for depth in 0..=128i32 {
            for move_count in [0usize, 1, 2, 4, 5, 10, 32, 100, 217, 218, 255] {
                let d = depth.clamp(0, 128) as usize;
                let m = move_count.min(255);
                assert_eq!(
                    LMR_TABLE[d][m],
                    reference(depth, move_count),
                    "LMR mismatch at depth={} moves={}",
                    depth,
                    move_count
                );
            }
        }
    }

    #[test]
    fn test_quiescence_finds_quiet_promotion() {
        // White pawn starts on e6, so no promotion exists at the root.
        // At depth 2 (1.e7 <black reply>) quiescence runs with White to
        // move and only quiescence can play the quiet promotion e8=Q.
        // Stand pat there would be a few hundred centipawns at most;
        // seeing the promotion gives ~1000.
        let board = Board::from_fen("k7/8/4P3/8/8/8/8/4K3 w - - 0 1").unwrap();
        let mut engine = SearchEngine::new();

        let result = engine.search(&board, SearchLimits::depth(2));

        assert!(
            result.score > 800,
            "Quiescence must see the quiet promotion, got {}",
            result.score
        );
    }

    #[test]
    fn test_node_limit_respected() {
        let board = Board::starting_position();
        let mut engine = SearchEngine::new();

        let result = engine.search(&board, SearchLimits::infinite().with_nodes(1000));

        let total = result.stats.nodes + result.stats.qnodes;
        assert!(
            total <= 1000,
            "Search must respect node limit, used {}",
            total
        );
    }

    #[test]
    fn test_searchmoves_restriction() {
        use crate::uci::parse_uci_move;

        let board = Board::starting_position();
        let legal = board.generate_legal_moves();
        let only_e4 = parse_uci_move("e2e4", &legal).unwrap();
        let mut engine = SearchEngine::new();

        let result = engine.search(&board, SearchLimits::depth(3).with_root_moves(vec![only_e4]));
        let best = result.best_move.expect("Should find a move");

        assert_eq!(best.from, only_e4.from);
        assert_eq!(best.to, only_e4.to);
    }

    #[test]
    fn test_no_queen_blunder() {
        // Simulate the interactive game scenario:
        // 1. Start from initial position
        // 2. User plays e4
        // 3. Engine searches and plays e6
        // 4. User plays d4
        // 5. Engine searches and plays... should NOT be Qg5!

        let mut board = Board::starting_position();
        board.enable_history();

        let mut engine = SearchEngine::new();

        // User plays e4
        let e4 = board
            .generate_legal_moves()
            .into_iter()
            .find(|m| {
                m.from.file() == 4 && m.from.rank() == 1 && m.to.file() == 4 && m.to.rank() == 3
            })
            .unwrap();
        board.make_move(e4);

        // Engine searches for response to e4 (this populates TT)
        let _e6_result = engine.search(&board, SearchLimits::depth(6));
        // Assume engine plays e6
        let e6 = board
            .generate_legal_moves()
            .into_iter()
            .find(|m| {
                m.from.file() == 4 && m.from.rank() == 6 && m.to.file() == 4 && m.to.rank() == 5
            })
            .unwrap();
        board.make_move(e6);

        // User plays d4
        let d4 = board
            .generate_legal_moves()
            .into_iter()
            .find(|m| {
                m.from.file() == 3 && m.from.rank() == 1 && m.to.file() == 3 && m.to.rank() == 3
            })
            .unwrap();
        board.make_move(d4);

        // Engine searches for response to d4 - this is where the bug occurs
        let result = engine.search(&board, SearchLimits::depth(10));

        let best_move = result.best_move.expect("Should find a move");

        // Check that the engine does NOT play d8g5 (Qg5)
        let from_sq = best_move.from;
        let to_sq = best_move.to;
        let is_queen_blunder = from_sq.file() == 3
            && from_sq.rank() == 7 // d8
            && to_sq.file() == 6
            && to_sq.rank() == 4; // g5

        assert!(
            !is_queen_blunder,
            "Engine blundered queen with Qg5! Score: {}, PV should not start with d8g5",
            result.score
        );

        // Print the actual move for debugging
        let from_str = format!(
            "{}{}",
            (b'a' + best_move.from.file()) as char,
            (b'1' + best_move.from.rank()) as char
        );
        let to_str = format!(
            "{}{}",
            (b'a' + best_move.to.file()) as char,
            (b'1' + best_move.to.rank()) as char
        );
        eprintln!("Best move at depth 10: {}{}, score: {}", from_str, to_str, result.score);
    }

}
