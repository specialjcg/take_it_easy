use tch::Tensor;

#[derive(Debug)]
pub struct MCTSResult {
    pub board_tensor: Tensor,
    pub best_position: usize,
    pub subscore: f64,
}

impl Clone for MCTSResult {
    fn clone(&self) -> Self {
        MCTSResult {
            board_tensor: self.board_tensor.shallow_clone(), // Manually clone the tensor
            best_position: self.best_position,
            subscore: self.subscore,
        }
    }
}