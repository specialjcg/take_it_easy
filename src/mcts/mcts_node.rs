use crate::game::game_state::GameState;

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct MCTSNode {
    pub state: GameState,              // Current game state
    pub visits: usize,                 // Number of visits
    pub value: f64,                    // Total value of the node
    pub children: Vec<MCTSNode>,       // Child nodes
    pub parent: Option<*mut MCTSNode>, // Pointer to the parent node (raw pointer to allow mutation)
    pub prior_probabilities: Option<Vec<f32>>, // Prior probabilities from neural network (19 positions)
}
