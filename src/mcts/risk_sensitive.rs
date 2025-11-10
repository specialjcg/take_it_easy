///! Risk-Sensitive MCTS using Conditional Value-at-Risk (CVaR)
///!
///! CVaR allows the MCTS to adapt its risk profile:
///! - alpha=0.0: Very prudent (minimize worst-case scenarios)
///! - alpha=0.5: Balanced (median-based evaluation)
///! - alpha=1.0: Aggressive (maximize best-case scenarios)
///!
///! This is particularly useful for Take It Easy where:
///! - Completing a line gives high rewards (risk worth taking)
///! - Breaking a line gives zero points (catastrophic failure to avoid)

/// Calculate risk-adjusted value using Conditional Value-at-Risk (CVaR)
///
/// # Arguments
/// * `values` - Vector of rollout scores for a given position
/// * `alpha` - Risk profile parameter ∈ [0.0, 1.0]
///   - 0.0: Prudent (focus on worst-case scenarios)
///   - 0.5: Balanced (median-based)
///   - 1.0: Aggressive (focus on best-case scenarios)
///
/// # Returns
/// Risk-adjusted value that can be used in place of mean score
///
/// # Example
/// ```
/// let scores = vec![100.0, 120.0, 80.0, 150.0, 90.0];
///
/// // Prudent strategy: focuses on avoiding bad outcomes
/// let prudent_value = cvar_risk_adjustment(&scores, 0.2);
///
/// // Balanced strategy: similar to median
/// let balanced_value = cvar_risk_adjustment(&scores, 0.5);
///
/// // Aggressive strategy: focuses on maximizing upside
/// let aggressive_value = cvar_risk_adjustment(&scores, 0.8);
/// ```
pub fn cvar_risk_adjustment(values: &[f64], alpha: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    if values.len() == 1 {
        return values[0];
    }

    // Clamp alpha to valid range
    let alpha = alpha.clamp(0.0, 1.0);

    // Sort values in ascending order
    let mut sorted_values = values.to_vec();
    sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Calculate cutoff index based on alpha
    // alpha=0.0 → index=0 (worst value only)
    // alpha=0.5 → index=len/2 (median)
    // alpha=1.0 → index=len-1 (all values, equivalent to mean)
    let cutoff_index = ((sorted_values.len() as f64) * alpha).ceil() as usize;
    let cutoff_index = cutoff_index.max(1).min(sorted_values.len());

    // CVaR: average of the alpha-quantile
    // For prudent (alpha < 0.5), we average the WORST alpha% of outcomes
    // For aggressive (alpha > 0.5), we average the BEST (1-alpha)% of outcomes

    if alpha < 0.5 {
        // Prudent: focus on tail risk (worst outcomes)
        let tail_values = &sorted_values[..cutoff_index];
        tail_values.iter().sum::<f64>() / tail_values.len() as f64
    } else {
        // Aggressive: focus on upside (best outcomes)
        let start_index = sorted_values.len() - cutoff_index;
        let upper_values = &sorted_values[start_index..];
        upper_values.iter().sum::<f64>() / upper_values.len() as f64
    }
}

/// Adaptive risk profile based on game context
///
/// Returns an alpha value that adapts to the current game state:
/// - Early game (turns 0-5): Balanced exploration (alpha=0.5)
/// - Mid game (turns 6-14): Slightly aggressive (alpha=0.6)
/// - Late game (turns 15-18): Prudent (alpha=0.3) to avoid breaking lines
///
/// # Arguments
/// * `current_turn` - Current turn number (0-18)
/// * `current_score` - Current score on the plateau
/// * `total_turns` - Total number of turns (usually 19)
///
/// # Returns
/// Recommended alpha value for CVaR calculation
pub fn adaptive_risk_profile(current_turn: usize, current_score: i32, total_turns: usize) -> f64 {
    let game_progress = current_turn as f64 / total_turns as f64;

    // Base alpha by game phase
    let base_alpha = if current_turn < 5 {
        // Early game: explore broadly (balanced)
        0.5
    } else if current_turn < 15 {
        // Mid game: slightly aggressive to build lines
        0.6
    } else {
        // Late game: prudent to avoid breaking lines
        0.35
    };

    // Adjust based on current performance
    let score_adjustment = if current_score < 60 {
        // Behind target: be more aggressive
        0.15
    } else if current_score > 100 {
        // Ahead of target: be more prudent to protect lead
        -0.15
    } else {
        // On target: stick to base strategy
        0.0
    };

    (base_alpha + score_adjustment).clamp(0.2, 0.8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cvar_prudent_strategy() {
        let scores = vec![50.0, 100.0, 150.0, 200.0, 250.0];

        // Prudent (alpha=0.2): should focus on lower 20% → [50.0] → 50.0
        let prudent = cvar_risk_adjustment(&scores, 0.2);
        assert!(prudent < 100.0, "Prudent strategy should focus on worst outcomes");
        assert!(prudent >= 50.0);
    }

    #[test]
    fn test_cvar_balanced_strategy() {
        let scores = vec![50.0, 100.0, 150.0, 200.0, 250.0];

        // Balanced (alpha=0.5): should be around median → ~150.0
        let balanced = cvar_risk_adjustment(&scores, 0.5);
        assert!((balanced - 150.0).abs() < 50.0, "Balanced strategy should be near median");
    }

    #[test]
    fn test_cvar_aggressive_strategy() {
        let scores = vec![50.0, 100.0, 150.0, 200.0, 250.0];

        // Aggressive (alpha=0.8): should focus on top 20% → [200.0, 250.0] → 225.0
        let aggressive = cvar_risk_adjustment(&scores, 0.8);
        assert!(aggressive > 180.0, "Aggressive strategy should focus on best outcomes");
    }

    #[test]
    fn test_cvar_ordering() {
        let scores = vec![50.0, 100.0, 150.0, 200.0, 250.0];

        let prudent = cvar_risk_adjustment(&scores, 0.2);
        let balanced = cvar_risk_adjustment(&scores, 0.5);
        let aggressive = cvar_risk_adjustment(&scores, 0.8);

        assert!(prudent < balanced, "Prudent < Balanced");
        assert!(balanced < aggressive, "Balanced < Aggressive");
    }

    #[test]
    fn test_cvar_edge_cases() {
        // Single value
        assert_eq!(cvar_risk_adjustment(&[100.0], 0.5), 100.0);

        // Empty vector
        assert_eq!(cvar_risk_adjustment(&[], 0.5), 0.0);

        // All same values
        let same = vec![150.0, 150.0, 150.0];
        assert_eq!(cvar_risk_adjustment(&same, 0.3), 150.0);
    }

    #[test]
    fn test_adaptive_risk_profile() {
        // Early game: balanced
        let early_alpha = adaptive_risk_profile(3, 30, 19);
        assert!((early_alpha - 0.5).abs() < 0.2, "Early game should be balanced");

        // Mid game: slightly aggressive
        let mid_alpha = adaptive_risk_profile(10, 80, 19);
        assert!(mid_alpha > 0.5, "Mid game should be slightly aggressive");

        // Late game: prudent
        let late_alpha = adaptive_risk_profile(17, 120, 19);
        assert!(late_alpha < 0.5, "Late game should be prudent");
    }

    #[test]
    fn test_adaptive_risk_behind_target() {
        // Behind target: should be more aggressive
        let behind_alpha = adaptive_risk_profile(10, 40, 19);
        let ontrack_alpha = adaptive_risk_profile(10, 80, 19);

        assert!(behind_alpha > ontrack_alpha, "Behind target should be more aggressive");
    }

    #[test]
    fn test_adaptive_risk_ahead_target() {
        // Ahead of target: should be more prudent
        let ahead_alpha = adaptive_risk_profile(10, 120, 19);
        let ontrack_alpha = adaptive_risk_profile(10, 80, 19);

        assert!(ahead_alpha < ontrack_alpha, "Ahead of target should be more prudent");
    }
}
