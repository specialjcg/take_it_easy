//! Hybridation entre la politique du Transformer et les heuristiques de boost
//! Permet de combiner les prédictions du réseau avec les règles manuelles

use tch::Tensor;

/// Configuration pour l'hybridation policy
#[derive(Debug, Clone)]
pub struct HybridConfig {
    /// Alpha : poids des heuristiques (0.0 = 100% Transformer, 1.0 = 100% heuristiques)
    pub alpha: f32,
    /// Activer le mode dynamique (ajuster alpha en fonction de la confiance du modèle)
    pub dynamic_alpha: bool,
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            alpha: 0.5, // 50/50 par défaut
            dynamic_alpha: false,
        }
    }
}

/// Combine une politique Transformer avec une politique heuristique (boostée)
///
/// # Arguments
/// * `transformer_policy` - Prédictions du Transformer (probabilités, 19 positions)
/// * `heuristic_policy` - Politique issue des boosts MCTS (probabilités, 19 positions)
/// * `config` - Configuration de l'hybridation
///
/// # Returns
/// Politique hybride combinant les deux sources
pub fn hybrid_policy(
    transformer_policy: &[f32],
    heuristic_policy: &[f32],
    config: &HybridConfig,
) -> Vec<f32> {
    assert_eq!(
        transformer_policy.len(),
        heuristic_policy.len(),
        "Policies must have same length"
    );

    let alpha = if config.dynamic_alpha {
        // Alpha dynamique basé sur l'entropie du Transformer
        let entropy = compute_entropy(transformer_policy);
        // Haute entropie (incertitude) → plus de poids sur heuristiques
        // Basse entropie (confiance) → plus de poids sur Transformer
        let normalized_entropy = (entropy / 3.0).clamp(0.0, 1.0);
        (config.alpha + normalized_entropy * 0.3).min(1.0)
    } else {
        config.alpha
    };

    transformer_policy
        .iter()
        .zip(heuristic_policy.iter())
        .map(|(&t_prob, &h_prob)| (1.0 - alpha) * t_prob + alpha * h_prob)
        .collect()
}

/// Calcule l'entropie d'une distribution de probabilité
fn compute_entropy(probs: &[f32]) -> f32 {
    probs
        .iter()
        .filter(|&&p| p > 1e-8)
        .map(|&p| -p * p.ln())
        .sum()
}

/// Applique l'hybridation sur des tensors PyTorch
pub fn hybrid_policy_tensor(
    transformer_logits: &Tensor,
    heuristic_probs: &Tensor,
    config: &HybridConfig,
) -> Tensor {
    use tch::Kind;

    // Convertir les logits du Transformer en probabilités
    let transformer_probs = transformer_logits.softmax(-1, Kind::Float);

    // Hybridation
    let alpha = config.alpha as f64;
    &transformer_probs * (1.0 - alpha) + heuristic_probs * alpha
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hybrid_policy_basic() {
        let transformer = vec![0.5, 0.3, 0.2];
        let heuristic = vec![0.1, 0.8, 0.1];
        let config = HybridConfig {
            alpha: 0.5,
            dynamic_alpha: false,
        };

        let result = hybrid_policy(&transformer, &heuristic, &config);

        // alpha=0.5 : moyenne 50/50
        assert!((result[0] - 0.3).abs() < 0.01); // (0.5+0.1)/2
        assert!((result[1] - 0.55).abs() < 0.01); // (0.3+0.8)/2
        assert!((result[2] - 0.15).abs() < 0.01); // (0.2+0.1)/2
    }

    #[test]
    fn test_hybrid_policy_full_transformer() {
        let transformer = vec![0.5, 0.3, 0.2];
        let heuristic = vec![0.1, 0.8, 0.1];
        let config = HybridConfig {
            alpha: 0.0, // 100% Transformer
            dynamic_alpha: false,
        };

        let result = hybrid_policy(&transformer, &heuristic, &config);

        assert!((result[0] - 0.5).abs() < 0.01);
        assert!((result[1] - 0.3).abs() < 0.01);
        assert!((result[2] - 0.2).abs() < 0.01);
    }

    #[test]
    fn test_hybrid_policy_full_heuristic() {
        let transformer = vec![0.5, 0.3, 0.2];
        let heuristic = vec![0.1, 0.8, 0.1];
        let config = HybridConfig {
            alpha: 1.0, // 100% heuristique
            dynamic_alpha: false,
        };

        let result = hybrid_policy(&transformer, &heuristic, &config);

        assert!((result[0] - 0.1).abs() < 0.01);
        assert!((result[1] - 0.8).abs() < 0.01);
        assert!((result[2] - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_entropy_calculation() {
        // Distribution uniforme → haute entropie
        let uniform = vec![0.33, 0.33, 0.34];
        let entropy_uniform = compute_entropy(&uniform);

        // Distribution concentrée → basse entropie
        let concentrated = vec![0.9, 0.05, 0.05];
        let entropy_concentrated = compute_entropy(&concentrated);

        assert!(entropy_uniform > entropy_concentrated);
    }
}
