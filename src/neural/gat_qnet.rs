//! GAT-based Q-Value Network for position ranking
//!
//! Uses Graph Attention Network instead of CNN for hexagonal geometry.
//! Trained to predict softmax distribution over positions for action pruning.

use crate::game::deck::Deck;
use crate::game::plateau::Plateau;
use crate::game::tile::Tile;
use crate::neural::gat::GraphAttentionNetwork;
use crate::neural::tensor_conversion::{convert_plateau_for_gat_47ch, convert_plateau_for_gat_extended};
use tch::{nn, Device, Kind, Tensor};

/// GAT-based Q-Value Network
pub struct GATQValueNet {
    encoder: GraphAttentionNetwork,
    head: nn::Linear,
    use_extended: bool, // 95ch vs 47ch
}

impl GATQValueNet {
    /// Create new GAT Q-Net
    ///
    /// # Arguments
    /// * `vs` - Variable store
    /// * `use_extended` - If true, use 95ch features; if false, use 47ch
    /// * `hidden_dims` - Hidden layer dimensions (e.g., [128, 128])
    /// * `num_heads` - Number of attention heads
    /// * `dropout` - Dropout rate
    pub fn new(
        vs: &nn::VarStore,
        use_extended: bool,
        hidden_dims: &[i64],
        num_heads: usize,
        dropout: f64,
    ) -> Self {
        let input_dim = if use_extended { 95 } else { 47 };
        let encoder = GraphAttentionNetwork::new(vs, input_dim, hidden_dims, num_heads, dropout);
        let out_dim = encoder.output_dim();
        let head = nn::linear(vs.root() / "qvalue_head", out_dim, 1, Default::default());

        Self {
            encoder,
            head,
            use_extended,
        }
    }

    /// Forward pass
    /// node_features: [batch, 19, input_dim]
    /// Returns: [batch, 19] Q-values (apply softmax externally for ranking)
    pub fn forward(&self, node_features: &Tensor, train: bool) -> Tensor {
        let h = self.encoder.forward(node_features, train);
        h.apply(&self.head).squeeze_dim(-1)
    }

    /// Predict ranking probabilities for all 19 positions
    pub fn predict_ranking(&self, plateau: &Plateau, tile: &Tile, deck: &Deck) -> [f64; 19] {
        let num_placed = plateau.tiles.iter().filter(|t| **t != Tile(0, 0, 0)).count();

        let features = if self.use_extended {
            convert_plateau_for_gat_extended(plateau, tile, deck, num_placed, 19)
        } else {
            convert_plateau_for_gat_47ch(plateau, tile, deck, num_placed, 19)
        };

        let input = features.unsqueeze(0); // [1, 19, channels]
        let output = self.forward(&input, false);
        let output_softmax = output.softmax(-1, Kind::Float);

        let mut probs = [0.0f64; 19];
        for i in 0..19 {
            probs[i] = output_softmax.double_value(&[0, i as i64]);
        }
        probs
    }

    /// Get top-K positions by ranking probability
    pub fn get_top_positions(&self, plateau: &Plateau, tile: &Tile, deck: &Deck, top_k: usize) -> Vec<usize> {
        let probs = self.predict_ranking(plateau, tile, deck);

        // Filter to empty positions only
        let mut scored: Vec<(usize, f64)> = plateau
            .tiles
            .iter()
            .enumerate()
            .filter(|(_, t)| **t == Tile(0, 0, 0))
            .map(|(pos, _)| (pos, probs[pos]))
            .collect();

        // Sort by probability descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Return top-K positions
        scored
            .iter()
            .take(top_k.min(scored.len()))
            .map(|(pos, _)| *pos)
            .collect()
    }
}

/// GAT Q-Net manager for loading and caching
pub struct GATQNetManager {
    #[allow(dead_code)]
    vs: nn::VarStore,
    net: GATQValueNet,
}

impl GATQNetManager {
    /// Create a new untrained GAT Q-Net
    pub fn new(use_extended: bool, hidden_dims: &[i64], num_heads: usize, dropout: f64) -> Self {
        let vs = nn::VarStore::new(Device::Cpu);
        let net = GATQValueNet::new(&vs, use_extended, hidden_dims, num_heads, dropout);
        Self { vs, net }
    }

    /// Load from saved weights
    pub fn load(
        model_path: &str,
        use_extended: bool,
        hidden_dims: &[i64],
        num_heads: usize,
        dropout: f64,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut vs = nn::VarStore::new(Device::Cpu);
        let net = GATQValueNet::new(&vs, use_extended, hidden_dims, num_heads, dropout);
        vs.load(model_path)?;
        Ok(Self { vs, net })
    }

    /// Save weights
    pub fn save(&self, model_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.vs.save(model_path)?;
        Ok(())
    }

    pub fn net(&self) -> &GATQValueNet {
        &self.net
    }

    pub fn vs_mut(&mut self) -> &mut nn::VarStore {
        &mut self.vs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::create_deck::create_deck;
    use crate::game::plateau::create_plateau_empty;

    #[test]
    fn test_gat_qnet_forward() {
        let vs = nn::VarStore::new(Device::Cpu);
        let net = GATQValueNet::new(&vs, false, &[64, 64], 4, 0.1);

        // Batch of 2, 19 nodes, 47 features
        let x = Tensor::randn([2, 19, 47], (Kind::Float, Device::Cpu));
        let out = net.forward(&x, false);

        assert_eq!(out.size(), vec![2, 19]);
    }

    #[test]
    fn test_gat_qnet_ranking() {
        let manager = GATQNetManager::new(false, &[64, 64], 4, 0.1);
        let plateau = create_plateau_empty();
        let deck = create_deck();
        let tile = Tile(1, 2, 3);

        let probs = manager.net().predict_ranking(&plateau, &tile, &deck);

        // All probabilities should be positive and sum to ~1
        let sum: f64 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 0.01, "Probs should sum to 1, got {}", sum);
    }

    #[test]
    fn test_gat_qnet_top_positions() {
        let manager = GATQNetManager::new(false, &[64, 64], 4, 0.1);
        let plateau = create_plateau_empty();
        let deck = create_deck();
        let tile = Tile(1, 2, 3);

        let top = manager.net().get_top_positions(&plateau, &tile, &deck, 5);

        assert_eq!(top.len(), 5);
        // All positions should be valid (0-18) and unique
        for &pos in &top {
            assert!(pos < 19);
        }
    }
}
