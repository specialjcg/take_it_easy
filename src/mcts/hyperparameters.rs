//! MCTS Hyperparameters Configuration
//!
//! This module defines all tunable hyperparameters for the MCTS algorithm.
//!
//! Optimization History:
//! - Original baseline: 147 pts
//! - Phase 1 (2025-11-07): Evaluation weights optimization → 158.05 pts (+11 pts, +7.5%)
//! - Quick Wins (2025-11-10): Temperature annealing optimization → 159.95 pts (+1.90 pts, +1.2%)
//! - Total improvement: 147 → 159.95 pts (+12.95 pts, +8.8%)

use serde::{Deserialize, Serialize};

/// MCTS hyperparameters configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCTSHyperparameters {
    // ========== c_puct (Exploration Constant) ==========
    /// c_puct for early game (turns 0-4)
    /// Higher values = more exploration
    /// Default: 4.2
    pub c_puct_early: f64,

    /// c_puct for mid game (turns 5-15)
    /// Default: 3.8
    pub c_puct_mid: f64,

    /// c_puct for late game (turns 16+)
    /// Lower values = more exploitation
    /// Default: 3.0
    pub c_puct_late: f64,

    /// Variance multiplier when uncertainty is high (variance > 0.5)
    /// Default: 1.3
    pub variance_mult_high: f64,

    /// Variance multiplier when uncertainty is very low (variance < 0.05)
    /// Default: 0.85
    pub variance_mult_low: f64,

    // ========== Dynamic Pruning ==========
    /// Pruning ratio for early game (turns 0-4)
    /// 0.05 = keep top 95% of moves
    /// Default: 0.05
    pub prune_early: f64,

    /// Pruning ratio for early-mid game (turns 5-9)
    /// Default: 0.10
    pub prune_mid1: f64,

    /// Pruning ratio for mid game (turns 10-14)
    /// Default: 0.15
    pub prune_mid2: f64,

    /// Pruning ratio for late game (turns 15+)
    /// Default: 0.20
    pub prune_late: f64,

    // ========== Adaptive Rollout Count ==========
    /// Rollouts for very strong moves (value > 0.7)
    /// Fewer rollouts needed when CNN is confident
    /// Default: 3
    pub rollout_strong: usize,

    /// Rollouts for medium-strong moves (0.2 < value <= 0.7)
    /// Default: 5
    pub rollout_medium: usize,

    /// Rollouts for neutral moves
    /// Default: 7
    pub rollout_default: usize,

    /// Rollouts for weak moves (value < -0.4)
    /// More rollouts to explore uncertain positions
    /// Default: 9
    pub rollout_weak: usize,

    // ========== Evaluation Weights (Pattern Rollouts V2) ==========
    /// Weight for CNN value network prediction
    /// Default: 0.6
    pub weight_cnn: f64,

    /// Weight for rollout simulation results
    /// Default: 0.2
    pub weight_rollout: f64,

    /// Weight for domain-specific heuristics
    /// Default: 0.1
    pub weight_heuristic: f64,

    /// Weight for contextual/entropy boost
    /// Default: 0.1
    pub weight_contextual: f64,

    // ========== Adaptive Simulations (Quick Win #1) ==========
    /// Simulation count multiplier for early game (turns 0-4)
    /// Lower values = fewer simulations
    /// Default: 0.67 (100 sims if base is 150)
    pub sim_mult_early: f64,

    /// Simulation count multiplier for mid game (turns 5-15)
    /// Default: 1.0 (150 sims if base is 150)
    pub sim_mult_mid: f64,

    /// Simulation count multiplier for late game (turns 16+)
    /// Higher values = more simulations for critical decisions
    /// Default: 1.67 (250 sims if base is 150)
    pub sim_mult_late: f64,

    // ========== Temperature Annealing (Quick Win #2) ==========
    /// Initial exploration temperature (early game)
    /// Higher values = more exploration
    /// Default: 1.5
    pub temp_initial: f64,

    /// Final exploitation temperature (late game)
    /// Lower values = more exploitation
    /// Default: 0.5
    pub temp_final: f64,

    /// Turn at which temperature starts decreasing
    /// Default: 5
    pub temp_decay_start: usize,

    /// Turn at which temperature reaches minimum
    /// Default: 15
    pub temp_decay_end: usize,

    // ========== RAVE (Rapid Action Value Estimation) ==========
    /// RAVE blending constant k for adaptive β calculation
    /// Formula: β = sqrt(k / (3*N + k)) where N = visit count
    /// Higher values = more influence from RAVE (All-Moves-As-First heuristic)
    /// Lower values = faster convergence to pure MCTS values
    /// Default: 10 (conservative, avoids early RAVE dominance)
    pub rave_k: f64,
}

impl Default for MCTSHyperparameters {
    fn default() -> Self {
        Self {
            // c_puct
            c_puct_early: 4.2,
            c_puct_mid: 3.8,
            c_puct_late: 3.0,
            variance_mult_high: 1.3,
            variance_mult_low: 0.85,

            // Pruning
            prune_early: 0.05,
            prune_mid1: 0.10,
            prune_mid2: 0.15,
            prune_late: 0.20,

            // Rollouts
            rollout_strong: 3,
            rollout_medium: 5,
            rollout_default: 7,
            rollout_weak: 9,

            // Evaluation weights (optimized Phase 1: 2025-11-07)
            // Phase 1 found: 158.05 pts vs 147 pts baseline (+11 pts, +7.5%)
            weight_cnn: 0.65,        // was 0.60
            weight_rollout: 0.25,    // was 0.20
            weight_heuristic: 0.05,  // was 0.10
            weight_contextual: 0.05, // was 0.10

            // Adaptive simulations (Quick Win #1)
            sim_mult_early: 0.67, // 100 sims
            sim_mult_mid: 1.0,    // 150 sims
            sim_mult_late: 1.67,  // 250 sims

            // Temperature annealing (Quick Win #2) - OPTIMIZED 2025-11-10
            // Grid search found: temp_initial=1.8, decay 7-13 performs best (159.95 pts)
            temp_initial: 1.8,   // was 1.5 → increased for more early exploration
            temp_final: 0.5,     // confirmed optimal
            temp_decay_start: 7, // was 5 → delayed start
            temp_decay_end: 13,  // was 15 → earlier finish

            // RAVE (Sprint 3)
            rave_k: 10.0, // Conservative constant to avoid early RAVE dominance
        }
    }
}

impl MCTSHyperparameters {
    /// Get c_puct value based on current turn
    pub fn get_c_puct(&self, current_turn: usize) -> f64 {
        if current_turn < 5 {
            self.c_puct_early
        } else if current_turn > 15 {
            self.c_puct_late
        } else {
            self.c_puct_mid
        }
    }

    /// Get variance multiplier based on variance level
    pub fn get_variance_multiplier(&self, variance: f64) -> f64 {
        if variance > 0.5 {
            self.variance_mult_high
        } else if variance > 0.2 {
            1.1
        } else if variance > 0.05 {
            1.0
        } else {
            self.variance_mult_low
        }
    }

    /// Get pruning ratio based on current turn
    pub fn get_pruning_ratio(&self, current_turn: usize) -> f64 {
        if current_turn < 5 {
            self.prune_early
        } else if current_turn < 10 {
            self.prune_mid1
        } else if current_turn < 15 {
            self.prune_mid2
        } else {
            self.prune_late
        }
    }

    /// Get rollout count based on value estimate
    pub fn get_rollout_count(&self, value_estimate: f64) -> usize {
        match value_estimate {
            x if x > 0.7 => self.rollout_strong,
            x if x > 0.2 => self.rollout_medium,
            x if x < -0.4 => self.rollout_weak,
            _ => self.rollout_default,
        }
    }

    /// Get adaptive simulation count based on current turn and base simulations
    /// Quick Win #1: More simulations for critical late-game decisions
    pub fn get_adaptive_simulations(&self, current_turn: usize, base_simulations: usize) -> usize {
        let multiplier = if current_turn < 5 {
            self.sim_mult_early
        } else if current_turn > 15 {
            self.sim_mult_late
        } else {
            self.sim_mult_mid
        };

        (base_simulations as f64 * multiplier).round() as usize
    }

    /// Get temperature for exploration/exploitation tradeoff
    /// Quick Win #2: Start with high exploration, end with pure exploitation
    pub fn get_temperature(&self, current_turn: usize) -> f64 {
        if current_turn < self.temp_decay_start {
            self.temp_initial
        } else if current_turn >= self.temp_decay_end {
            self.temp_final
        } else {
            // Linear interpolation
            let progress = (current_turn - self.temp_decay_start) as f64
                / (self.temp_decay_end - self.temp_decay_start) as f64;
            self.temp_initial + progress * (self.temp_final - self.temp_initial)
        }
    }

    /// Validate that evaluation weights sum to approximately 1.0
    #[allow(dead_code)] // Used in binaries, not in lib
    pub fn validate_weights(&self) -> Result<(), String> {
        let sum =
            self.weight_cnn + self.weight_rollout + self.weight_heuristic + self.weight_contextual;

        if (sum - 1.0).abs() > 0.01 {
            Err(format!(
                "Evaluation weights must sum to 1.0, got {:.3}",
                sum
            ))
        } else {
            Ok(())
        }
    }

    /// Create a configuration string for logging
    #[allow(dead_code)] // Used in binaries, not in lib
    pub fn to_config_string(&self) -> String {
        format!(
            "c_puct[{:.2},{:.2},{:.2}]_prune[{:.2},{:.2},{:.2},{:.2}]_roll[{},{},{},{}]_weights[{:.2},{:.2},{:.2},{:.2}]",
            self.c_puct_early,
            self.c_puct_mid,
            self.c_puct_late,
            self.prune_early,
            self.prune_mid1,
            self.prune_mid2,
            self.prune_late,
            self.rollout_strong,
            self.rollout_medium,
            self.rollout_default,
            self.rollout_weak,
            self.weight_cnn,
            self.weight_rollout,
            self.weight_heuristic,
            self.weight_contextual
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_weights_sum_to_one() {
        let params = MCTSHyperparameters::default();
        assert!(params.validate_weights().is_ok());
    }

    #[test]
    fn test_get_c_puct_by_turn() {
        let params = MCTSHyperparameters::default();
        assert_eq!(params.get_c_puct(0), 4.2); // Early
        assert_eq!(params.get_c_puct(4), 4.2); // Early
        assert_eq!(params.get_c_puct(5), 3.8); // Mid
        assert_eq!(params.get_c_puct(15), 3.8); // Mid
        assert_eq!(params.get_c_puct(16), 3.0); // Late
    }

    #[test]
    fn test_get_rollout_count() {
        let params = MCTSHyperparameters::default();
        assert_eq!(params.get_rollout_count(0.8), 3); // Strong
        assert_eq!(params.get_rollout_count(0.5), 5); // Medium
        assert_eq!(params.get_rollout_count(0.0), 7); // Default
        assert_eq!(params.get_rollout_count(-0.5), 9); // Weak
    }

    #[test]
    fn test_invalid_weights() {
        let mut params = MCTSHyperparameters::default();
        params.weight_cnn = 0.9; // Sum > 1.0
        assert!(params.validate_weights().is_err());
    }

    #[test]
    fn test_config_string() {
        let params = MCTSHyperparameters::default();
        let config = params.to_config_string();
        assert!(config.contains("c_puct[4.20,3.80,3.00]"));
        // Updated after Phase 1 optimization: 0.65,0.25,0.05,0.05
        assert!(config.contains("weights[0.65,0.25,0.05,0.05]"));
    }
}
