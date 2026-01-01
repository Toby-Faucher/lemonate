//! Time Management
//!
//! Controls search time allocation and termination.
//! Supports various time control modes: fixed depth, fixed time, and game clocks.
//!
//! # Implementation Notes
//! - Use soft and hard time limits for iterative deepening
//! - Support sudden death and increment time controls
//! - Allow early termination when best move is stable

use std::time::{Duration, Instant};

/// Default number of moves to assume remaining in sudden death.
const DEFAULT_MOVES_TO_GO: u32 = 30;

/// Fraction of remaining time to use as base allocation.
const TIME_FRACTION: f64 = 0.05;

/// Maximum fraction of remaining time for hard limit.
const MAX_TIME_FRACTION: f64 = 0.25;

/// Soft limit as fraction of hard limit.
const SOFT_LIMIT_FRACTION: f64 = 0.6;

/// Minimum time to allocate (1 millisecond).
const MIN_TIME_MS: u64 = 1;

/// Time control mode for the search.
#[derive(Clone, Debug)]
pub enum TimeControl {
    /// Search to a fixed depth.
    FixedDepth(u8),
    /// Search for a fixed amount of time.
    FixedTime(Duration),
    /// Game clock with remaining time and optional increment.
    GameClock {
        /// Remaining time on our clock.
        remaining: Duration,
        /// Time increment per move (0 for sudden death).
        increment: Duration,
        /// Estimated moves until time control (None for sudden death).
        moves_to_go: Option<u32>,
    },
    /// Infinite search until stopped.
    Infinite,
}

/// Search limits and constraints.
#[derive(Clone, Debug)]
pub struct SearchLimits {
    /// Time control mode.
    pub time_control: TimeControl,
    /// Maximum depth to search (optional).
    pub max_depth: Option<u8>,
    /// Maximum nodes to search (optional).
    pub max_nodes: Option<u64>,
}

impl SearchLimits {
    /// Create limits for a fixed depth search.
    pub fn depth(depth: u8) -> Self {
        Self {
            time_control: TimeControl::FixedDepth(depth),
            max_depth: Some(depth),
            max_nodes: None,
        }
    }

    /// Create limits for a fixed time search.
    pub fn movetime(time_ms: u64) -> Self {
        Self {
            time_control: TimeControl::FixedTime(Duration::from_millis(time_ms)),
            max_depth: None,
            max_nodes: None,
        }
    }

    /// Create limits for infinite search.
    pub fn infinite() -> Self {
        Self {
            time_control: TimeControl::Infinite,
            max_depth: None,
            max_nodes: None,
        }
    }

    /// Create limits for a game clock.
    pub fn game_clock(remaining_ms: u64, increment_ms: u64, moves_to_go: Option<u32>) -> Self {
        Self {
            time_control: TimeControl::GameClock {
                remaining: Duration::from_millis(remaining_ms),
                increment: Duration::from_millis(increment_ms),
                moves_to_go,
            },
            max_depth: None,
            max_nodes: None,
        }
    }

    /// Add a node limit to existing limits.
    pub fn with_nodes(mut self, nodes: u64) -> Self {
        self.max_nodes = Some(nodes);
        self
    }

    /// Add a depth limit to existing limits.
    pub fn with_depth(mut self, depth: u8) -> Self {
        self.max_depth = Some(depth);
        self
    }
}

impl Default for SearchLimits {
    fn default() -> Self {
        Self::infinite()
    }
}

/// Manages time allocation and search termination.
pub struct TimeManager {
    /// When the search started.
    start_time: Instant,
    /// Soft time limit (prefer to stop here).
    soft_limit: Duration,
    /// Hard time limit (must stop here).
    hard_limit: Duration,
    /// Original soft limit (before adjustments).
    base_soft_limit: Duration,
    /// Original hard limit (before adjustments).
    base_hard_limit: Duration,
    /// Whether the search has been stopped.
    stopped: bool,
    /// Nodes searched (for node limit checking).
    nodes_searched: u64,
    /// Node limit (if any).
    node_limit: Option<u64>,
    /// Whether time limits apply (false for depth/infinite).
    use_time_limit: bool,
}

impl TimeManager {
    /// Create a new time manager with the given limits.
    pub fn new(limits: &SearchLimits) -> Self {
        let (soft_limit, hard_limit, use_time_limit) = match &limits.time_control {
            TimeControl::FixedDepth(_) => {
                // No time limit for fixed depth.
                (Duration::MAX, Duration::MAX, false)
            }
            TimeControl::FixedTime(duration) => {
                // Soft limit is a fraction of the specified time.
                let hard = *duration;
                let soft_ms = (hard.as_millis() as f64 * SOFT_LIMIT_FRACTION) as u64;
                let soft = Duration::from_millis(soft_ms.max(MIN_TIME_MS));
                (soft, hard, true)
            }
            TimeControl::GameClock {
                remaining,
                increment,
                moves_to_go,
            } => {
                let (soft, hard) = Self::calculate_time_budget(*remaining, *increment, *moves_to_go);
                (soft, hard, true)
            }
            TimeControl::Infinite => {
                // No time limit for infinite search.
                (Duration::MAX, Duration::MAX, false)
            }
        };

        Self {
            start_time: Instant::now(),
            soft_limit,
            hard_limit,
            base_soft_limit: soft_limit,
            base_hard_limit: hard_limit,
            stopped: false,
            nodes_searched: 0,
            node_limit: limits.max_nodes,
            use_time_limit,
        }
    }

    /// Start the clock for a new search.
    pub fn start(&mut self) {
        self.start_time = Instant::now();
        self.stopped = false;
        self.nodes_searched = 0;
    }

    /// Check if we should stop searching.
    ///
    /// Call this periodically during search (e.g., every 1024 nodes).
    #[inline]
    pub fn should_stop(&self) -> bool {
        // Check stop flag first (fastest).
        if self.stopped {
            return true;
        }

        // Check node limit.
        if let Some(limit) = self.node_limit {
            if self.nodes_searched >= limit {
                return true;
            }
        }

        // Check hard time limit.
        if self.use_time_limit && self.elapsed() >= self.hard_limit {
            return true;
        }

        false
    }

    /// Check if we can start another iteration of iterative deepening.
    ///
    /// Returns true if we have enough time for another iteration.
    /// Uses the soft limit to make this decision.
    #[inline]
    pub fn can_start_iteration(&self) -> bool {
        if self.stopped {
            return false;
        }

        if !self.use_time_limit {
            return true;
        }

        // Use soft limit for iteration decisions.
        self.elapsed() < self.soft_limit
    }

    /// Signal that the search should stop immediately.
    pub fn stop(&mut self) {
        self.stopped = true;
    }

    /// Check if the search has been manually stopped.
    #[inline]
    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    /// Get elapsed time since search start.
    #[inline]
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get elapsed time in milliseconds.
    #[inline]
    pub fn elapsed_ms(&self) -> u128 {
        self.elapsed().as_millis()
    }

    /// Update node count for node limit checking.
    #[inline]
    pub fn add_nodes(&mut self, nodes: u64) {
        self.nodes_searched += nodes;
    }

    /// Get the current node count.
    #[inline]
    pub fn nodes(&self) -> u64 {
        self.nodes_searched
    }

    /// Calculate time allocation for a game clock.
    ///
    /// Uses a simple but effective formula:
    /// - Base time = remaining / moves_to_go
    /// - Add a fraction of the increment
    /// - Soft limit = base time * SOFT_LIMIT_FRACTION
    /// - Hard limit = min(base time, remaining * MAX_TIME_FRACTION)
    ///
    /// # Arguments
    /// * `remaining` - Time remaining on clock
    /// * `increment` - Time increment per move
    /// * `moves_to_go` - Moves until time control (None for sudden death)
    fn calculate_time_budget(
        remaining: Duration,
        increment: Duration,
        moves_to_go: Option<u32>,
    ) -> (Duration, Duration) {
        let remaining_ms = remaining.as_millis() as f64;
        let increment_ms = increment.as_millis() as f64;

        // Estimate moves remaining.
        let moves = moves_to_go.unwrap_or(DEFAULT_MOVES_TO_GO) as f64;

        // Base allocation: remaining time divided by expected moves.
        let base_time_ms = remaining_ms * TIME_FRACTION + remaining_ms / moves;

        // Add increment (use most of it, save a bit for overhead).
        let with_increment_ms = base_time_ms + increment_ms * 0.9;

        // Hard limit: don't use more than MAX_TIME_FRACTION of remaining time.
        let max_time_ms = remaining_ms * MAX_TIME_FRACTION;
        let hard_limit_ms = with_increment_ms.min(max_time_ms).max(MIN_TIME_MS as f64);

        // Soft limit: fraction of hard limit.
        let soft_limit_ms = (hard_limit_ms * SOFT_LIMIT_FRACTION).max(MIN_TIME_MS as f64);

        (
            Duration::from_millis(soft_limit_ms as u64),
            Duration::from_millis(hard_limit_ms as u64),
        )
    }

    /// Adjust time allocation based on position complexity.
    ///
    /// Spend more time on complex/critical positions.
    /// Factor > 1.0 increases time, < 1.0 decreases time.
    pub fn adjust_for_complexity(&mut self, factor: f32) {
        if !self.use_time_limit {
            return;
        }

        let factor = factor.clamp(0.5, 2.0) as f64;

        // Adjust soft limit (main control for iterations).
        let new_soft_ms = (self.base_soft_limit.as_millis() as f64 * factor) as u64;
        self.soft_limit = Duration::from_millis(new_soft_ms.max(MIN_TIME_MS));

        // Adjust hard limit (but don't exceed original hard limit).
        let new_hard_ms = (self.base_hard_limit.as_millis() as f64 * factor) as u64;
        let capped_hard_ms = new_hard_ms.min(self.base_hard_limit.as_millis() as u64);
        self.hard_limit = Duration::from_millis(capped_hard_ms.max(MIN_TIME_MS));
    }

    /// Adjust time allocation based on best move stability.
    ///
    /// If best move hasn't changed across iterations, we can stop earlier.
    /// More consecutive iterations with the same best move = more reduction.
    pub fn adjust_for_stability(&mut self, same_move_count: u32) {
        if !self.use_time_limit || same_move_count < 4 {
            return;
        }

        // Reduce soft limit based on stability.
        // 4 iterations same move: 90% time
        // 5 iterations same move: 80% time
        // 6+ iterations same move: 70% time
        let factor = match same_move_count {
            4 => 0.9,
            5 => 0.8,
            _ => 0.7,
        };

        let new_soft_ms = (self.soft_limit.as_millis() as f64 * factor) as u64;
        self.soft_limit = Duration::from_millis(new_soft_ms.max(MIN_TIME_MS));
    }

    /// Get the soft time limit.
    pub fn soft_limit(&self) -> Duration {
        self.soft_limit
    }

    /// Get the hard time limit.
    pub fn hard_limit(&self) -> Duration {
        self.hard_limit
    }

    /// Reset adjustments to base values.
    pub fn reset_adjustments(&mut self) {
        self.soft_limit = self.base_soft_limit;
        self.hard_limit = self.base_hard_limit;
    }
}

impl Default for TimeManager {
    fn default() -> Self {
        Self {
            start_time: Instant::now(),
            soft_limit: Duration::MAX,
            hard_limit: Duration::MAX,
            base_soft_limit: Duration::MAX,
            base_hard_limit: Duration::MAX,
            stopped: false,
            nodes_searched: 0,
            node_limit: None,
            use_time_limit: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_fixed_depth_no_time_limit() {
        let limits = SearchLimits::depth(10);
        let tm = TimeManager::new(&limits);

        assert!(!tm.use_time_limit);
        assert!(tm.can_start_iteration());
        assert!(!tm.should_stop());
    }

    #[test]
    fn test_infinite_no_time_limit() {
        let limits = SearchLimits::infinite();
        let tm = TimeManager::new(&limits);

        assert!(!tm.use_time_limit);
        assert!(tm.can_start_iteration());
        assert!(!tm.should_stop());
    }

    #[test]
    fn test_fixed_time() {
        let limits = SearchLimits::movetime(100); // 100ms
        let tm = TimeManager::new(&limits);

        assert!(tm.use_time_limit);
        assert!(tm.can_start_iteration());
        assert!(!tm.should_stop());

        // Wait for time to expire.
        thread::sleep(Duration::from_millis(110));
        assert!(tm.should_stop());
    }

    #[test]
    fn test_game_clock_allocation() {
        // 1 minute remaining, 1 second increment, 20 moves to go.
        let limits = SearchLimits::game_clock(60_000, 1_000, Some(20));
        let tm = TimeManager::new(&limits);

        assert!(tm.use_time_limit);

        // Should allocate reasonable time (not too little, not too much).
        let soft_ms = tm.soft_limit().as_millis();
        let hard_ms = tm.hard_limit().as_millis();

        assert!(soft_ms > 0, "Soft limit should be positive");
        assert!(hard_ms > soft_ms, "Hard limit should exceed soft limit");
        assert!(
            hard_ms <= 15_000,
            "Hard limit should not exceed 25% of remaining: {}",
            hard_ms
        );
    }

    #[test]
    fn test_game_clock_sudden_death() {
        // 30 seconds remaining, no increment, sudden death.
        let limits = SearchLimits::game_clock(30_000, 0, None);
        let tm = TimeManager::new(&limits);

        let hard_ms = tm.hard_limit().as_millis();

        // Should be conservative in sudden death.
        assert!(
            hard_ms <= 7_500,
            "Should be conservative in sudden death: {}",
            hard_ms
        );
    }

    #[test]
    fn test_game_clock_with_increment() {
        // 10 seconds remaining, 5 second increment.
        let limits = SearchLimits::game_clock(10_000, 5_000, None);
        let tm = TimeManager::new(&limits);

        let hard_ms = tm.hard_limit().as_millis();

        // Should use some of the increment.
        assert!(
            hard_ms > 500,
            "Should use some of the increment: {}",
            hard_ms
        );
    }

    #[test]
    fn test_stop_signal() {
        let limits = SearchLimits::infinite();
        let mut tm = TimeManager::new(&limits);

        assert!(!tm.should_stop());
        assert!(!tm.is_stopped());

        tm.stop();

        assert!(tm.should_stop());
        assert!(tm.is_stopped());
        assert!(!tm.can_start_iteration());
    }

    #[test]
    fn test_node_limit() {
        let limits = SearchLimits::infinite().with_nodes(1000);
        let mut tm = TimeManager::new(&limits);

        assert!(!tm.should_stop());

        tm.add_nodes(500);
        assert!(!tm.should_stop());
        assert_eq!(tm.nodes(), 500);

        tm.add_nodes(500);
        assert!(tm.should_stop());
        assert_eq!(tm.nodes(), 1000);
    }

    #[test]
    fn test_elapsed_time() {
        let limits = SearchLimits::infinite();
        let tm = TimeManager::new(&limits);

        thread::sleep(Duration::from_millis(10));

        let elapsed = tm.elapsed_ms();
        assert!(elapsed >= 10, "Elapsed should be at least 10ms: {}", elapsed);
    }

    #[test]
    fn test_start_resets_timer() {
        let limits = SearchLimits::infinite();
        let mut tm = TimeManager::new(&limits);

        thread::sleep(Duration::from_millis(20));
        let elapsed1 = tm.elapsed_ms();

        tm.start();
        let elapsed2 = tm.elapsed_ms();

        assert!(elapsed1 >= 20);
        assert!(elapsed2 < 10, "Timer should be reset: {}", elapsed2);
    }

    #[test]
    fn test_start_clears_stop_flag() {
        let limits = SearchLimits::infinite();
        let mut tm = TimeManager::new(&limits);

        tm.stop();
        assert!(tm.is_stopped());

        tm.start();
        assert!(!tm.is_stopped());
    }

    #[test]
    fn test_adjust_for_complexity() {
        let limits = SearchLimits::movetime(1000);
        let mut tm = TimeManager::new(&limits);

        let original_soft = tm.soft_limit();

        // Increase complexity.
        tm.adjust_for_complexity(1.5);
        assert!(tm.soft_limit() > original_soft);

        // Reset and decrease complexity.
        tm.reset_adjustments();
        tm.adjust_for_complexity(0.5);
        assert!(tm.soft_limit() < original_soft);
    }

    #[test]
    fn test_adjust_for_stability() {
        let limits = SearchLimits::movetime(1000);
        let mut tm = TimeManager::new(&limits);

        let original_soft = tm.soft_limit();

        // Few iterations - no change.
        tm.adjust_for_stability(2);
        assert_eq!(tm.soft_limit(), original_soft);

        // Many iterations with same move - should reduce.
        tm.adjust_for_stability(6);
        assert!(tm.soft_limit() < original_soft);
    }

    #[test]
    fn test_limits_with_depth() {
        let limits = SearchLimits::movetime(1000).with_depth(10);

        assert_eq!(limits.max_depth, Some(10));
        assert!(matches!(limits.time_control, TimeControl::FixedTime(_)));
    }

    #[test]
    fn test_limits_with_nodes() {
        let limits = SearchLimits::infinite().with_nodes(50000);

        assert_eq!(limits.max_nodes, Some(50000));
        assert!(matches!(limits.time_control, TimeControl::Infinite));
    }

    #[test]
    fn test_can_start_iteration_respects_soft_limit() {
        let limits = SearchLimits::movetime(50); // 50ms
        let tm = TimeManager::new(&limits);

        assert!(tm.can_start_iteration());

        // Wait past soft limit (60% of 50ms = 30ms).
        thread::sleep(Duration::from_millis(35));
        assert!(!tm.can_start_iteration());
    }

    #[test]
    fn test_minimum_time_allocation() {
        // Very short time - should still allocate at least MIN_TIME_MS.
        let limits = SearchLimits::game_clock(10, 0, Some(100));
        let tm = TimeManager::new(&limits);

        assert!(tm.soft_limit().as_millis() >= MIN_TIME_MS as u128);
        assert!(tm.hard_limit().as_millis() >= MIN_TIME_MS as u128);
    }
}
