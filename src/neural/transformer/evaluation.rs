use super::{
    attention::{KeyTensor, QueryTensor, ValueTensor},
    TransformerError, TransformerModel,
};
use crate::game::game_state::GameState;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tch::{Device, Kind, Tensor};

pub trait GameStateEval {
    fn to_feature_vector(&self) -> Vec<f32>;
    fn is_valid_move(&self, pos: usize) -> bool;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub inference_time_ms: f64,
    pub throughput: f64,
    pub memory_usage_mb: f64,
    pub accuracy: f64,
    pub mcts_node_count: usize,
    pub cache_hit_rate: f64,
}

pub struct PatternAnalysis {
    attention_patterns: Vec<Vec<f32>>,
    position_importance: Vec<f32>,
}

impl PatternAnalysis {
    pub fn new(patterns: Vec<Vec<f32>>, importance: Vec<f32>) -> Self {
        Self {
            attention_patterns: patterns,
            position_importance: importance,
        }
    }
}

pub struct TransformerEvaluator {
    model: TransformerModel,
}

impl TransformerEvaluator {
    pub fn new(model: TransformerModel) -> Self {
        Self { model }
    }

    pub fn analyze_patterns(
        &self,
        states: &[impl GameStateEval],
    ) -> Result<PatternAnalysis, TransformerError> {
        let weights = self.get_attention_weights(states)?;
        let importance = self.compute_position_importance(&weights);

        Ok(PatternAnalysis::new(
            self.convert_weights_to_vec(&weights)?,
            importance,
        ))
    }

    fn get_attention_weights(
        &self,
        states: &[impl GameStateEval],
    ) -> Result<Vec<Vec<Tensor>>, TransformerError> {
        let batch_tensor = self.states_to_tensor(states)?;
        let mut weights = Vec::new();

        for layer in self.model.layers() {
            let layer_weights = layer.attention.get_attention_weights(
                QueryTensor(batch_tensor.shallow_clone()),
                KeyTensor(batch_tensor.shallow_clone()),
                ValueTensor(batch_tensor.shallow_clone()),
            )?;
            weights.push(vec![layer_weights]);
        }

        Ok(weights)
    }

    fn states_to_tensor(&self, states: &[impl GameStateEval]) -> Result<Tensor, TransformerError> {
        let mut batch_features = Vec::new();

        for state in states {
            let state_features = state.to_feature_vector();
            // Les features sont maintenant 256 éléments plats
            // On reshape en [1, 256] pour le modèle
            batch_features.extend(state_features);
        }

        // Create tensor of shape [batch, 1, 256]
        let batch = states.len() as i64;
        Ok(Tensor::from_slice(&batch_features)
            .reshape(&[batch, 1, 256])
            .to_device(Device::Cpu))
    }

    fn compute_position_importance(&self, weights: &[Vec<Tensor>]) -> Vec<f32> {
        let mut importance = vec![0.0; 19];

        for layer_weights in weights {
            for weight_matrix in layer_weights {
                if let Ok(pos_importance) = weight_matrix
                    .mean_dim(&[-1i64][..], true, Kind::Float)
                    .to_kind(Kind::Float)
                    .try_into() as Result<Vec<f32>, _>
                {
                    for (i, &value) in pos_importance.iter().enumerate() {
                        importance[i] += value;
                    }
                }
            }
        }

        // Normalisation des importances
        if let Some(&max_val) = importance.iter().max_by(|a, b| a.partial_cmp(b).unwrap()) {
            if max_val > 0.0 {
                importance.iter_mut().for_each(|x| *x /= max_val);
            }
        }

        importance
    }

    fn convert_weights_to_vec(
        &self,
        weights: &[Vec<Tensor>],
    ) -> Result<Vec<Vec<f32>>, TransformerError> {
        let mut result: Vec<Vec<f32>> = Vec::with_capacity(weights.len());

        for layer in weights {
            let mut layer_vec: Vec<f32> = Vec::new();
            for tensor in layer {
                let flattened = tensor.flatten(0, -1);
                let vec_result: Result<Vec<f32>, _> = flattened.try_into();
                let v = vec_result.map_err(|e| {
                    TransformerError::TensorConversionError(format!(
                        "Failed to convert tensor: {}",
                        e
                    ))
                })?;
                layer_vec.extend(v);
            }
            result.push(layer_vec);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::super::TransformerConfig;
    use super::*;

    struct MockGameState {
        features: Vec<f32>,
    }

    impl GameStateEval for MockGameState {
        fn to_feature_vector(&self) -> Vec<f32> {
            self.features.clone()
        }

        fn is_valid_move(&self, _pos: usize) -> bool {
            true
        }
    }

    fn create_test_states() -> Vec<MockGameState> {
        vec![
            MockGameState {
                features: vec![0.0; 57], // 57 features: 19 positions × 3 values each
            },
            MockGameState {
                features: vec![1.0; 57],
            },
        ]
    }

    #[test]
    fn test_evaluator_creation() {
        let config = TransformerConfig::default();
        let vs = tch::nn::VarStore::new(tch::Device::Cpu);
        let model = TransformerModel::new(config, &vs.root()).unwrap();
        let evaluator = TransformerEvaluator::new(model);

        let test_states = create_test_states();
        let analysis = evaluator.analyze_patterns(&test_states);
        assert!(analysis.is_ok());
    }

    #[test]
    fn test_pattern_analysis() {
        let config = TransformerConfig::default();
        let vs = tch::nn::VarStore::new(tch::Device::Cpu);
        let model = TransformerModel::new(config, &vs.root()).unwrap();
        let evaluator = TransformerEvaluator::new(model);

        let test_states = create_test_states();
        let analysis = evaluator.analyze_patterns(&test_states).unwrap();

        assert_eq!(analysis.position_importance.len(), 19);
        assert!(analysis
            .position_importance
            .iter()
            .all(|&x| x >= 0.0 && x <= 1.0));
    }

    #[test]
    fn test_tensor_conversion() {
        let config = TransformerConfig::default();
        let vs = tch::nn::VarStore::new(tch::Device::Cpu);
        let model = TransformerModel::new(config, &vs.root()).unwrap();
        let evaluator = TransformerEvaluator::new(model);

        let test_states = create_test_states();
        let tensor_result = evaluator.states_to_tensor(&test_states);

        assert!(tensor_result.is_ok());
        if let Ok(tensor) = tensor_result {
            let size = tensor.size();
            assert_eq!(size[0], 2); // batch size
            assert_eq!(size[1], 19); // sequence length (19 positions)
            assert_eq!(size[2], 64); // feature dimension
        }
    }
}
