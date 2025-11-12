//! Gumbel MCTS Selection Strategy
//!
//! Implements Gumbel-Top-k sampling for better exploration in MCTS.
//! Based on "Policy improvement by planning with Gumbel" (Danihelka et al., 2022)
//!
//! Key innovation: Replace UCB with Gumbel noise for action selection
//! Formula: action = argmax_a [Q(s,a) + Gumbel(0,1) / temperature]
//!
//! Advantages over standard UCB:
//! - Better exploration of rare but promising branches
//! - Theoretically proven convergence for stochastic games
//! - Used in MuZero Reanalyze

use rand::{rng, Rng};
use std::collections::HashMap;

/// Samples from standard Gumbel(0,1) distribution
///
/// Gumbel(0,1) = -ln(-ln(Uniform(0,1)))
#[inline]
fn sample_gumbel(rng: &mut impl Rng) -> f64 {
    let u: f64 = rng.random_range(0.001..1.0);
    -(-(u.ln())).ln()
}

/// Gumbel MCTS selection strategy
pub struct GumbelSelector {
    /// Temperature parameter (higher = more exploration)
    /// Typical range: 0.5 to 2.0
    /// - temperature = 1.0: balanced
    /// - temperature < 1.0: more exploitation
    /// - temperature > 1.0: more exploration
    pub temperature: f64,
}

impl GumbelSelector {
    /// Creates a new Gumbel selector with given temperature
    pub fn new(temperature: f64) -> Self {
        Self { temperature }
    }

    /// Selects best move using Gumbel-Top-k sampling
    ///
    /// # Arguments
    /// * `q_values` - HashMap of position → Q-value (average reward)
    /// * `visit_counts` - HashMap of position → visit count
    /// * `k` - Number of top candidates to consider (typically 3-5)
    ///
    /// # Returns
    /// Selected position index
    pub fn select_action(
        &self,
        q_values: &HashMap<usize, f64>,
        visit_counts: &HashMap<usize, usize>,
        k: usize,
    ) -> Option<usize> {
        if q_values.is_empty() {
            return None;
        }

        let mut rng_instance = rng();
        let mut scored_moves: Vec<(usize, f64)> = Vec::new();

        for (&position, &q_value) in q_values.iter() {
            // Sample Gumbel noise
            let gumbel_noise = sample_gumbel(&mut rng_instance);

            // Gumbel score = Q(s,a) + Gumbel / temperature
            let gumbel_score = q_value + (gumbel_noise / self.temperature);

            // Optional: Add visit count bonus for unvisited nodes
            let visit_bonus = if let Some(&visits) = visit_counts.get(&position) {
                if visits == 0 {
                    10.0 // Large bonus for unvisited nodes
                } else {
                    0.0
                }
            } else {
                10.0
            };

            let final_score = gumbel_score + visit_bonus;
            scored_moves.push((position, final_score));
        }

        // Sort by Gumbel score (descending)
        scored_moves.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Select from top-k
        let top_k = usize::min(k, scored_moves.len());
        if top_k == 0 {
            return None;
        }

        // Return the best scoring move
        Some(scored_moves[0].0)
    }

    /// Adaptive temperature based on game progression
    ///
    /// # Arguments
    /// * `current_turn` - Current turn number
    /// * `total_turns` - Total turns in game
    ///
    /// # Returns
    /// Adjusted temperature (higher early, lower late)
    pub fn adaptive_temperature(current_turn: usize, total_turns: usize) -> f64 {
        let progress = current_turn as f64 / total_turns as f64;

        // Temperature schedule:
        // - Early game (0-33%): temperature = 1.5 (high exploration)
        // - Mid game (33-66%): temperature = 1.0 (balanced)
        // - Late game (66-100%): temperature = 0.5 (low exploration, more exploitation)

        if progress < 0.33 {
            1.5
        } else if progress < 0.66 {
            1.0
        } else {
            0.5
        }
    }
}

/// Complete Gumbel Selection: score + Gumbel noise
///
/// This function combines Q-value, visit count, and Gumbel noise for selection
///
/// # Arguments
/// * `q_values` - HashMap of position → average score
/// * `visit_counts` - HashMap of position → number of visits
/// * `temperature` - Exploration temperature
/// * `top_k` - Number of top candidates to consider
///
/// # Returns
/// Selected position
pub fn gumbel_select(
    q_values: &HashMap<usize, f64>,
    visit_counts: &HashMap<usize, usize>,
    temperature: f64,
    top_k: usize,
) -> Option<usize> {
    let selector = GumbelSelector::new(temperature);
    selector.select_action(q_values, visit_counts, top_k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gumbel_distribution() {
        let mut rng_instance = rng();

        // Sample 1000 values using sample_gumbel function
        let samples: Vec<f64> = (0..1000)
            .map(|_| sample_gumbel(&mut rng_instance))
            .collect();

        // Gumbel(0,1) has mean ≈ 0.577 (Euler-Mascheroni constant)
        let mean: f64 = samples.iter().sum::<f64>() / samples.len() as f64;
        assert!(
            (mean - 0.577).abs() < 0.1,
            "Mean should be ~0.577, got {}",
            mean
        );
    }

    #[test]
    fn test_gumbel_selector_basic() {
        let mut q_values = HashMap::new();
        q_values.insert(0, 0.5);
        q_values.insert(1, 0.7);
        q_values.insert(2, 0.3);

        let mut visit_counts = HashMap::new();
        visit_counts.insert(0, 10);
        visit_counts.insert(1, 5);
        visit_counts.insert(2, 1);

        let selector = GumbelSelector::new(1.0);
        let selected = selector.select_action(&q_values, &visit_counts, 3);

        assert!(selected.is_some());
        let pos = selected.unwrap();
        assert!(pos < 3);
    }

    #[test]
    fn test_gumbel_favors_unvisited() {
        let mut q_values = HashMap::new();
        q_values.insert(0, 0.5);
        q_values.insert(1, 0.5); // Same Q-value
        q_values.insert(2, 0.5); // Same Q-value

        let mut visit_counts = HashMap::new();
        visit_counts.insert(0, 100); // Heavily visited
        visit_counts.insert(1, 0); // Unvisited
        visit_counts.insert(2, 50); // Moderately visited

        let selector = GumbelSelector::new(1.0);

        // Run multiple times, position 1 (unvisited) should be selected frequently
        let mut selections = HashMap::new();
        for _ in 0..100 {
            let selected = selector.select_action(&q_values, &visit_counts, 3).unwrap();
            *selections.entry(selected).or_insert(0) += 1;
        }

        // Position 1 should have most selections due to visit bonus
        let pos1_count = selections.get(&1).unwrap_or(&0);
        assert!(
            *pos1_count > 30,
            "Unvisited node should be selected often, got {}",
            pos1_count
        );
    }

    #[test]
    fn test_adaptive_temperature() {
        // Early game: high temperature
        let temp_early = GumbelSelector::adaptive_temperature(3, 19);
        assert!((temp_early - 1.5).abs() < 0.01);

        // Mid game: medium temperature
        let temp_mid = GumbelSelector::adaptive_temperature(10, 19);
        assert!((temp_mid - 1.0).abs() < 0.01);

        // Late game: low temperature
        let temp_late = GumbelSelector::adaptive_temperature(17, 19);
        assert!((temp_late - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_temperature_effect() {
        let mut q_values = HashMap::new();
        q_values.insert(0, 1.0); // Best move
        q_values.insert(1, 0.5);
        q_values.insert(2, 0.0);

        let mut visit_counts = HashMap::new();
        visit_counts.insert(0, 10);
        visit_counts.insert(1, 10);
        visit_counts.insert(2, 10);

        // High temperature (more random)
        let selector_high = GumbelSelector::new(2.0);
        let mut selections_high = HashMap::new();
        for _ in 0..100 {
            let s = selector_high
                .select_action(&q_values, &visit_counts, 3)
                .unwrap();
            *selections_high.entry(s).or_insert(0) += 1;
        }

        // Low temperature (more greedy)
        let selector_low = GumbelSelector::new(0.1);
        let mut selections_low = HashMap::new();
        for _ in 0..100 {
            let s = selector_low
                .select_action(&q_values, &visit_counts, 3)
                .unwrap();
            *selections_low.entry(s).or_insert(0) += 1;
        }

        // Low temperature should select position 0 (best) more often
        let low_temp_best = selections_low.get(&0).unwrap_or(&0);
        let high_temp_best = selections_high.get(&0).unwrap_or(&0);

        assert!(
            *low_temp_best > *high_temp_best,
            "Low temp should be more greedy: low={}, high={}",
            low_temp_best,
            high_temp_best
        );
    }
}
