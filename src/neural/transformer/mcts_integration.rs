use super::{TransformerError, TransformerModel, POLICY_OUTPUTS};
use crate::game::game_state::GameState;
use crate::mcts::mcts_node::MCTSNode;
use crate::neural::transformer::game_state::GameStateFeatures;
use std::result::Result;
use std::sync::Arc;
use tch::{Device, Kind, Tensor};

#[derive(Debug)]
pub enum PredictionError {
    TransformerError(TransformerError),
    EncodingError(String),
    InvalidOutput(String),
}

impl From<TransformerError> for PredictionError {
    fn from(err: TransformerError) -> Self {
        PredictionError::TransformerError(err)
    }
}

pub trait MCTSInterface {
    fn get_state(&self) -> &GameState;
    fn set_prior_probability(&mut self, pos: usize, prob: f32);
    fn set_value(&mut self, value: f32);
}

impl MCTSInterface for MCTSNode {
    fn get_state(&self) -> &GameState {
        &self.state
    }

    fn set_prior_probability(&mut self, pos: usize, prob: f32) {
        if self.prior_probabilities.is_none() {
            self.prior_probabilities = Some(vec![0.0; 19]); // 19 positions
        }
        if let Some(ref mut priors) = self.prior_probabilities {
            if pos < priors.len() {
                priors[pos] = prob;
            }
        }
    }

    fn set_value(&mut self, value: f32) {
        self.value = value as f64;
    }
}

pub struct ParallelTransformerMCTS {
    model: Arc<TransformerModel>,
    device: Device,
}

impl ParallelTransformerMCTS {
    pub fn new(model: TransformerModel) -> Self {
        Self::with_device(model, Device::Cpu)
    }

    pub fn with_device(model: TransformerModel, device: Device) -> Self {
        Self {
            model: Arc::new(model),
            device,
        }
    }

    pub fn parallel_predict_batch(
        &self,
        states: &[&GameState],
    ) -> Result<Vec<(Vec<f32>, f32)>, TransformerError> {
        let batch_tensor = self.encode_batch(states)?;
        let (policy_logits, value_tensor) = self
            .model
            .infer(&batch_tensor)
            .map_err(|e| TransformerError::OptimizationError(e.to_string()))?;

        self.decode_predictions(&policy_logits, &value_tensor)
    }

    fn encode_batch(&self, states: &[&GameState]) -> Result<Tensor, TransformerError> {
        let mut features = Vec::new();

        for state in states {
            features.extend(state.to_tensor_features());
        }

        Ok(Tensor::from_slice(&features)
            .reshape(&[states.len() as i64, -1])
            .to_device(self.device))
    }

    fn decode_predictions(
        &self,
        policy_logits: &Tensor,
        value_tensor: &Tensor,
    ) -> Result<Vec<(Vec<f32>, f32)>, TransformerError> {
        let batch_size = policy_logits.size()[0];
        if batch_size == 0 {
            return Ok(Vec::new());
        }

        let policy_probs = policy_logits.softmax(-1, Kind::Float);
        let mut results = Vec::with_capacity(batch_size as usize);

        for i in 0..batch_size {
            let sample = policy_probs.get(i);
            let mut policy = Vec::with_capacity(POLICY_OUTPUTS as usize);
            for idx in 0..POLICY_OUTPUTS {
                policy.push(sample.double_value(&[idx]) as f32);
            }

            let value = if value_tensor.dim() == 2 {
                value_tensor.double_value(&[i, 0]) as f32
            } else {
                value_tensor.double_value(&[i]) as f32
            };

            results.push((policy, value));
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::game_state::GameState;
    use crate::neural::transformer::TransformerConfig;

    fn create_test_state() -> GameState {
        use crate::game::deck::Deck;
        use crate::game::plateau::create_plateau_empty;
        use crate::game::tile::Tile;
        let mut state = GameState {
            plateau: create_plateau_empty(),
            deck: Deck {
                tiles: vec![Tile(1, 2, 3)],
            },
        };
        // Place une tuile pour simuler un état non-vide
        state.plateau.tiles[0] = Tile(1, 2, 3);
        state
    }

    #[test]
    fn test_parallel_prediction() {
        let vs = tch::nn::VarStore::new(tch::Device::Cpu);
        let config = TransformerConfig::new(64, 2, 2).unwrap();
        let model = TransformerModel::new(config, &vs.root()).unwrap();
        let parallel_mcts = ParallelTransformerMCTS::new(model);

        let test_state1 = create_test_state();
        let test_state2 = create_test_state();
        let states = vec![&test_state1, &test_state2];
        let predictions = parallel_mcts.parallel_predict_batch(&states);

        assert!(predictions.is_ok());
        if let Ok(preds) = predictions {
            assert!(!preds.is_empty());
            for (policy, value) in preds.iter() {
                assert!(!policy.is_empty());
                assert!(value.is_finite());
            }
        }
    }

    #[test]
    fn test_mcts_integration() {
        let vs = tch::nn::VarStore::new(tch::Device::Cpu);
        let config = TransformerConfig::new(64, 2, 2).unwrap();
        let model = TransformerModel::new(config, &vs.root()).unwrap();
        let parallel_mcts = ParallelTransformerMCTS::new(model);

        let test_state = create_test_state();
        let states = vec![&test_state];
        let predictions = parallel_mcts.parallel_predict_batch(&states);

        assert!(predictions.is_ok());
        if let Ok(preds) = predictions {
            assert!(!preds.is_empty());
            let (policy, value) = &preds[0];
            assert!(!policy.is_empty());
            assert!(value.is_finite());
        }
    }
}
