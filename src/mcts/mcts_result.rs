use crate::game::plateau::Plateau;
use tch::Tensor;

#[derive(Debug)]
pub struct MCTSResult {
    pub board_tensor: Tensor,
    pub best_position: usize,
    pub subscore: f64,
    pub policy_distribution: Tensor,
    pub policy_distribution_boosted: Tensor,
    pub boost_intensity: f32,
    pub graph_features: Option<Tensor>, // Ajout pour debug/traçabilité GNN
    pub plateau: Option<Plateau>,       // Correction : plateau Rust
    pub current_turn: Option<usize>,
    pub total_turns: Option<usize>,
}

impl Clone for MCTSResult {
    fn clone(&self) -> Self {
        MCTSResult {
            board_tensor: self.board_tensor.shallow_clone(),
            best_position: self.best_position,
            subscore: self.subscore,
            policy_distribution: self.policy_distribution.shallow_clone(),
            policy_distribution_boosted: self.policy_distribution_boosted.shallow_clone(),
            boost_intensity: self.boost_intensity,
            graph_features: self.graph_features.as_ref().map(|t| t.shallow_clone()),
            plateau: self.plateau.clone(),
            current_turn: self.current_turn,
            total_turns: self.total_turns,
        }
    }
}
