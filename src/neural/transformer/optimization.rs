use super::{TransformerError, TransformerModel};
use crate::game::game_state::GameState as BaseGameState;
use crate::mcts::mcts_node::MCTSNode;
use std::sync::Arc;
use tch::{Device, Kind, Tensor};

pub trait MCTSNodeInterface {
    fn get_game_state(&self) -> &BaseGameState;
    fn set_prior_probabilities(&mut self, probs: &[f32]);
    fn set_value_estimate(&mut self, value: f32);
}

impl MCTSNodeInterface for MCTSNode {
    fn get_game_state(&self) -> &BaseGameState {
        &self.state
    }

    fn set_prior_probabilities(&mut self, _probs: &[f32]) {
        // Champ prior_probabilities inexistant, à implémenter si besoin
        // Pour l'instant, ne rien faire ou loguer un avertissement
    }

    fn set_value_estimate(&mut self, value: f32) {
        self.value = value as f64;
    }
}

#[derive(Clone, Debug)]
pub struct OptimizedConfig {
    pub device: Device,
    pub batch_size: i64,
    pub sequence_length: i64,
    pub use_mixed_precision: bool,
}

impl Default for OptimizedConfig {
    fn default() -> Self {
        Self {
            device: Device::Cpu,
            batch_size: 32,
            sequence_length: 512,
            use_mixed_precision: false,
        }
    }
}

pub struct OptimizedTransformer {
    config: Arc<OptimizedConfig>,
    model: TransformerModel,
}

impl OptimizedTransformer {
    pub fn new(model: TransformerModel, config: OptimizedConfig) -> Result<Self, TransformerError> {
        Ok(Self {
            model,
            config: Arc::new(config),
        })
    }

    pub fn forward_optimized(&self, input: &Tensor) -> Result<Tensor, TransformerError> {
        let input = input.to_device(self.config.device);

        let output = if self.config.use_mixed_precision {
            self.forward_mixed_precision(&input)?
        } else {
            self.model
                .forward(&input)
                .map_err(|e| TransformerError::OptimizationError(e.to_string()))?
        };

        Ok(output.to_device(Device::Cpu))
    }

    fn forward_mixed_precision(&self, input: &Tensor) -> Result<Tensor, TransformerError> {
        let input_fp16 = input.to_kind(Kind::Half);
        // Clone du modèle et conversion des poids en Half
        let mut model_fp16 = self.model.clone();
        for layer in &mut model_fp16.layers {
            layer.ff1.ws = layer.ff1.ws.to_kind(Kind::Half);
            if let Some(ref mut bs) = layer.ff1.bs {
                let bs_half: Tensor = bs.to_kind(Kind::Half);
                *bs = bs_half;
            }
            layer.ff2.ws = layer.ff2.ws.to_kind(Kind::Half);
            if let Some(ref mut bs) = layer.ff2.bs {
                let bs_half: Tensor = bs.to_kind(Kind::Half);
                *bs = bs_half;
            }
            // AttentionLayer conversion
            layer.attention.linear_q.ws = layer.attention.linear_q.ws.to_kind(Kind::Half);
            if let Some(ref mut bs) = layer.attention.linear_q.bs {
                let bs_half: Tensor = bs.to_kind(Kind::Half);
                *bs = bs_half;
            }
            layer.attention.linear_k.ws = layer.attention.linear_k.ws.to_kind(Kind::Half);
            if let Some(ref mut bs) = layer.attention.linear_k.bs {
                let bs_half: Tensor = bs.to_kind(Kind::Half);
                *bs = bs_half;
            }
            layer.attention.linear_v.ws = layer.attention.linear_v.ws.to_kind(Kind::Half);
            if let Some(ref mut bs) = layer.attention.linear_v.bs {
                let bs_half: Tensor = bs.to_kind(Kind::Half);
                *bs = bs_half;
            }
            layer.attention.linear_out.ws = layer.attention.linear_out.ws.to_kind(Kind::Half);
            if let Some(ref mut bs) = layer.attention.linear_out.bs {
                let bs_half: Tensor = bs.to_kind(Kind::Half);
                *bs = bs_half;
            }
        }
        let output_fp16 = model_fp16
            .forward(&input_fp16)
            .map_err(|e| TransformerError::OptimizationError(e.to_string()))?;
        Ok(output_fp16.to_kind(Kind::Float))
    }

    pub fn get_memory_stats(&self) -> MemoryStats {
        let device_stats = self.get_device_stats();
        MemoryStats {
            allocated: device_stats.0,
            cached: device_stats.1,
            max_allocated: device_stats.2,
        }
    }

    fn get_device_stats(&self) -> (usize, usize, usize) {
        // Les fonctions CUDA ne sont pas disponibles dans tch::utils
        // TODO: Ajouter une implémentation si une API existe dans le futur
        (0, 0, 0)
    }

    pub fn optimize_for_inference(&self, input: &Tensor) -> Result<Tensor, TransformerError> {
        let device_tensor = input.to_device(self.config.device);

        let optimized = if let Device::Cuda(_) = self.config.device {
            device_tensor.pin_memory(self.config.device)
        } else {
            device_tensor
        };

        Ok(optimized)
    }
}

#[derive(Debug, Default)]
pub struct MemoryStats {
    pub allocated: usize,
    pub cached: usize,
    pub max_allocated: usize,
}

#[cfg(test)]
mod tests {
    use super::super::TransformerConfig;
    use super::*;
    use tch::nn;

    #[test]
    fn test_optimized_transformer_creation() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let model = TransformerModel::new(TransformerConfig::default(), &vs.root()).unwrap();
        let config = OptimizedConfig::default();

        let optimized = OptimizedTransformer::new(model, config);
        assert!(optimized.is_ok());
    }

    #[test]
    fn test_mixed_precision_forward() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let model = TransformerModel::new(TransformerConfig::default(), &vs.root()).unwrap();
        let config = OptimizedConfig {
            use_mixed_precision: true,
            ..Default::default()
        };

        let optimized = OptimizedTransformer::new(model, config).unwrap();
        let input = Tensor::rand(&[1, 4, 64], (Kind::Float, Device::Cpu));

        let output = optimized.forward_optimized(&input);
        assert!(output.is_ok());

        if let Ok(output) = output {
            assert_eq!(output.size(), input.size());
            assert_eq!(output.kind(), Kind::Float);
        }
    }

    #[test]
    fn test_memory_tracking() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let model = TransformerModel::new(TransformerConfig::default(), &vs.root()).unwrap();
        let config = OptimizedConfig::default();

        let optimized = OptimizedTransformer::new(model, config).unwrap();
        let stats = optimized.get_memory_stats();

        // Les stats devraient être à 0 pour CPU
        assert_eq!(stats.allocated, 0);
        assert_eq!(stats.cached, 0);
        assert_eq!(stats.max_allocated, 0);
    }

    #[test]
    fn test_optimization_basic() {
        let vs = tch::nn::VarStore::new(tch::Device::Cpu);
        let model = TransformerModel::new(TransformerConfig::default(), &vs.root()).unwrap();
        let config = OptimizedConfig::default();

        let optimized = OptimizedTransformer::new(model, config);
        assert!(optimized.is_ok());
    }
    #[test]
    fn test_optimization_batch() {
        let vs = tch::nn::VarStore::new(tch::Device::Cpu);
        let model = TransformerModel::new(TransformerConfig::default(), &vs.root()).unwrap();
        let config = OptimizedConfig {
            batch_size: 64,
            ..Default::default()
        };

        let optimized = OptimizedTransformer::new(model, config).unwrap();
        let input = Tensor::rand(&[64, 4, 64], (Kind::Float, Device::Cpu));

        let output = optimized.forward_optimized(&input);
        assert!(output.is_ok());

        if let Ok(output) = output {
            assert_eq!(output.size(), input.size());
            assert_eq!(output.kind(), Kind::Float);
        }
    }
    #[test]
    fn test_optimization_memory() {
        let vs = tch::nn::VarStore::new(tch::Device::Cpu);
        let model = TransformerModel::new(TransformerConfig::default(), &vs.root()).unwrap();
        let config = OptimizedConfig::default();

        let optimized = OptimizedTransformer::new(model, config).unwrap();
        let stats = optimized.get_memory_stats();

        // Les stats devraient être à 0 pour CPU
        assert_eq!(stats.allocated, 0);
        assert_eq!(stats.cached, 0);
        assert_eq!(stats.max_allocated, 0);
    }
}
