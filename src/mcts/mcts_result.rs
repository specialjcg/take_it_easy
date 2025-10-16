use tch::Tensor;

#[derive(Debug)]
pub struct MCTSResult {
    pub board_tensor: Tensor,
    pub best_position: usize,
    pub subscore: f64,
    pub policy_distribution: Tensor,
    pub policy_distribution_boosted: Tensor,
    pub boost_intensity: f32,
}

impl Clone for MCTSResult {
    fn clone(&self) -> Self {
        MCTSResult {
            board_tensor: self.board_tensor.shallow_clone(), // Manually clone the tensor
            best_position: self.best_position,
            subscore: self.subscore,
            policy_distribution: self.policy_distribution.shallow_clone(),
            policy_distribution_boosted: self.policy_distribution_boosted.shallow_clone(),
            boost_intensity: self.boost_intensity,
        }
    }
}
