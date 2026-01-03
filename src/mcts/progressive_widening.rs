
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
