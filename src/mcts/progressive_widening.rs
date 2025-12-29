///! Progressive Widening for MCTS
///!
///! Progressive Widening (PW) dynamically limits the number of actions explored
///! based on the number of visits to a node. This technique is particularly effective
///! for games with large branching factors like Take It Easy (19 positions × variable tiles).
///!
///! Key Formula: k(n) = C × n^α
///! - n: number of visits to the current node
///! - C: constant controlling initial exploration (default: 1.0-2.0)
///! - α: parameter controlling widening rate (default: 0.25-0.5)
///!   - α=0.25: Conservative widening (slow growth)
///!   - α=0.5: Moderate widening (square root growth)
///!   - α=1.0: Linear widening (aggressive)
///!
///! Benefits:
///! - Reduces computational waste on unlikely actions
///! - Focuses simulations on promising moves
///! - Improves sample efficiency with limited simulation budget
///! - Adapts exploration breadth to confidence level

use std::collections::HashMap;

/// Progressive Widening configuration
#[derive(Debug, Clone)]
pub struct ProgressiveWideningConfig {
    /// Constant C in k(n) = C × n^α
    pub c_constant: f64,
    /// Exponent α in k(n) = C × n^α
    pub alpha: f64,
    /// Minimum number of actions to always consider (safety threshold)
    pub min_actions: usize,
}

impl Default for ProgressiveWideningConfig {
    fn default() -> Self {
        Self {
            c_constant: 1.5,
            alpha: 0.4,
            min_actions: 3,
        }
    }
}

impl ProgressiveWideningConfig {
    /// Conservative configuration: slow widening, focus on exploitation
    pub fn conservative() -> Self {
        Self {
            c_constant: 1.0,
            alpha: 0.25,
            min_actions: 2,
        }
    }

    /// Balanced configuration: moderate widening
    pub fn balanced() -> Self {
        Self::default()
    }

    /// Aggressive configuration: fast widening, more exploration
    pub fn aggressive() -> Self {
        Self {
            c_constant: 2.0,
            alpha: 0.5,
            min_actions: 5,
        }
    }

    /// Adaptive configuration based on game phase
    pub fn adaptive(current_turn: usize, total_turns: usize) -> Self {
        let progress = current_turn as f64 / total_turns as f64;

        if progress < 0.3 {
            // Early game: more exploration
            Self::aggressive()
        } else if progress < 0.7 {
            // Mid game: balanced
            Self::balanced()
        } else {
            // Late game: focus on best moves
            Self::conservative()
        }
    }
}

/// Calculate the maximum number of actions to consider given visit count
///
/// Formula: k(n) = min(total_actions, max(min_actions, C × n^α))
///
/// # Arguments
/// * `visits` - Number of times this node has been visited
/// * `total_actions` - Total number of legal actions available
/// * `config` - Progressive widening configuration
///
/// # Returns
/// Maximum number of actions to explore
///
/// # Example
/// ```
/// use take_it_easy::mcts::progressive_widening::{ProgressiveWideningConfig, max_actions_to_explore};
///
/// let config = ProgressiveWideningConfig::default();
///
/// // First visit: explore min_actions
/// assert_eq!(max_actions_to_explore(1, 19, &config), 3);
///
/// // After 10 visits: k(10) = 1.5 × 10^0.4 ≈ 3.77 → ceil(3.77) = 4
/// assert_eq!(max_actions_to_explore(10, 19, &config), 4);
///
/// // After 100 visits: k(100) = 1.5 × 100^0.4 ≈ 9.48 → ceil(9.48) = 10
/// assert_eq!(max_actions_to_explore(100, 19, &config), 10);
///
/// // After 1000 visits: k(1000) = 1.5 × 1000^0.4 ≈ 18.9 → ceil(18.9) = 19
/// assert_eq!(max_actions_to_explore(1000, 19, &config), 19);
/// ```
pub fn max_actions_to_explore(
    visits: usize,
    total_actions: usize,
    config: &ProgressiveWideningConfig,
) -> usize {
    if visits == 0 {
        return config.min_actions.min(total_actions);
    }

    let k = config.c_constant * (visits as f64).powf(config.alpha);
    let k_rounded = k.ceil() as usize;

    // Ensure we respect bounds: [min_actions, total_actions]
    k_rounded
        .max(config.min_actions)
        .min(total_actions)
}

/// Select top-k actions based on policy scores or value estimates
///
/// # Arguments
/// * `actions` - List of available actions with their scores
/// * `k` - Maximum number of actions to select
///
/// # Returns
/// Vector of top-k actions sorted by descending score
pub fn select_top_k_actions<T: Copy>(
    actions: &[(T, f64)],
    k: usize,
) -> Vec<T> {
    let mut sorted_actions: Vec<_> = actions.to_vec();

    // Sort by score descending
    sorted_actions.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Take top k
    sorted_actions
        .into_iter()
        .take(k)
        .map(|(action, _score)| action)
        .collect()
}

/// Track action exploration statistics for Progressive Widening
#[derive(Debug, Clone, Default)]
pub struct ActionExplorationTracker {
    /// Number of times each action has been explored
    pub action_visits: HashMap<usize, usize>,
    /// Total visits to the parent node
    pub total_visits: usize,
}

impl ActionExplorationTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a visit to an action
    pub fn record_visit(&mut self, action: usize) {
        *self.action_visits.entry(action).or_insert(0) += 1;
        self.total_visits += 1;
    }

    /// Get the number of visits for a specific action
    pub fn get_visits(&self, action: usize) -> usize {
        *self.action_visits.get(&action).unwrap_or(&0)
    }

    /// Check if an action should be added to the tree
    pub fn should_add_action(
        &self,
        available_actions: usize,
        config: &ProgressiveWideningConfig,
    ) -> bool {
        let current_explored = self.action_visits.len();
        let max_allowed = max_actions_to_explore(
            self.total_visits,
            available_actions,
            config,
        );

        current_explored < max_allowed
    }

    /// Get the set of currently explored actions
    pub fn explored_actions(&self) -> Vec<usize> {
        self.action_visits.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_actions_growth() {
        let config = ProgressiveWideningConfig::default(); // C=1.5, α=0.4

        // k(1) = 1.5 × 1^0.4 = 1.5 → max(3, 1) = 3
        assert_eq!(max_actions_to_explore(1, 19, &config), 3);

        // k(10) = 1.5 × 10^0.4 ≈ 3.77 → ceil(3.77) = 4
        assert_eq!(max_actions_to_explore(10, 19, &config), 4);

        // k(100) = 1.5 × 100^0.4 ≈ 9.48 → ceil(9.48) = 10
        assert_eq!(max_actions_to_explore(100, 19, &config), 10);

        // k(500) = 1.5 × 500^0.4 ≈ 19.03 → ceil(19.03) = 20 → min(20, 19) = 19
        assert_eq!(max_actions_to_explore(500, 19, &config), 19);

        // k(1000) = 1.5 × 1000^0.4 ≈ 18.9 → ceil(18.9) = 19 (equals total)
        assert_eq!(max_actions_to_explore(1000, 19, &config), 19);
    }

    #[test]
    fn test_conservative_vs_aggressive() {
        let conservative = ProgressiveWideningConfig::conservative();
        let aggressive = ProgressiveWideningConfig::aggressive();

        let visits = 100;
        let total = 19;

        let conservative_k = max_actions_to_explore(visits, total, &conservative);
        let aggressive_k = max_actions_to_explore(visits, total, &aggressive);

        assert!(
            conservative_k < aggressive_k,
            "Conservative should explore fewer actions than aggressive"
        );
    }

    #[test]
    fn test_select_top_k() {
        let actions = vec![
            (0, 0.1),
            (1, 0.9),
            (2, 0.3),
            (3, 0.7),
            (4, 0.5),
        ];

        let top3 = select_top_k_actions(&actions, 3);
        assert_eq!(top3, vec![1, 3, 4]); // Scores: 0.9, 0.7, 0.5

        let top2 = select_top_k_actions(&actions, 2);
        assert_eq!(top2, vec![1, 3]); // Scores: 0.9, 0.7

        let all = select_top_k_actions(&actions, 10);
        assert_eq!(all.len(), 5); // Only 5 actions available
    }

    #[test]
    fn test_action_exploration_tracker() {
        let mut tracker = ActionExplorationTracker::new();
        let config = ProgressiveWideningConfig::default();

        // Initially, should add actions
        assert!(tracker.should_add_action(19, &config));

        // Add first action
        tracker.record_visit(5);
        assert_eq!(tracker.get_visits(5), 1);
        assert_eq!(tracker.total_visits, 1);

        // With 1 visit, can explore up to 3 actions (min_actions)
        assert!(tracker.should_add_action(19, &config));

        tracker.record_visit(10);
        tracker.record_visit(15);

        // Now have 3 actions explored with 3 total visits
        // k(3) = 1.5 × 3^0.4 ≈ 2.3 → max(3, 2) = 3
        // Already exploring 3 actions, so should not add more yet
        assert!(!tracker.should_add_action(19, &config));

        // After more visits, should allow more actions
        for _ in 0..20 {
            tracker.record_visit(5);
        }
        // Now total_visits = 23, k(23) ≈ 6.2 → can explore 6 actions
        assert!(tracker.should_add_action(19, &config));
    }

    #[test]
    fn test_adaptive_config() {
        let early = ProgressiveWideningConfig::adaptive(3, 19); // Turn 3/19 ≈ 15%
        let mid = ProgressiveWideningConfig::adaptive(10, 19); // Turn 10/19 ≈ 52%
        let late = ProgressiveWideningConfig::adaptive(17, 19); // Turn 17/19 ≈ 89%

        // Early should be aggressive (higher alpha)
        // Late should be conservative (lower alpha)
        assert!(early.alpha >= mid.alpha);
        assert!(mid.alpha >= late.alpha);
    }

    #[test]
    fn test_bounds_respected() {
        let config = ProgressiveWideningConfig {
            c_constant: 100.0, // Very large C
            alpha: 2.0,        // Very large alpha
            min_actions: 3,
        };

        // Even with large C and α, should never exceed total_actions
        assert_eq!(max_actions_to_explore(100, 19, &config), 19);

        // Should never go below min_actions
        let small_config = ProgressiveWideningConfig {
            c_constant: 0.1,
            alpha: 0.1,
            min_actions: 5,
        };
        assert_eq!(max_actions_to_explore(1, 19, &small_config), 5);
    }
}
