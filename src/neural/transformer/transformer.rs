use std::result::Result;
use tch::{Tensor, Kind, Device};
use super::attention::{AttentionLayer, AttentionConfig, AttentionError};

#[derive(Debug)]
pub enum TransformerError {
    AttentionError(AttentionError),
    ConfigError(String),
    ForwardError(String),
}

impl From<AttentionError> for TransformerError {
    fn from(err: AttentionError) -> Self {
        TransformerError::AttentionError(err)
    }
}

#[derive(Clone)]
pub struct TransformerConfig {
    attention: AttentionConfig,
    ff_dim: i64,
    dropout: Option<f64>,
}

impl TransformerConfig {
    pub fn new(dim: i64, num_heads: i64, ff_dim: i64) -> Result<Self, TransformerError> {
        let attention = AttentionConfig::new(dim, num_heads)
            .map_err(TransformerError::AttentionError)?;

        if ff_dim <= 0 {
            return Err(TransformerError::ConfigError("FF dimension must be positive".into()));
        }

        Ok(Self {
            attention,
            ff_dim,
            dropout: None,
        })
    }

    pub fn with_dropout(self, dropout: f64) -> Result<Self, TransformerError> {
        let attention = self.attention
            .clone()
            .with_dropout(dropout)
            .map_err(TransformerError::AttentionError)?;

        Ok(Self {
            attention,
            dropout: Some(dropout),
            ..self
        })
    }
}

pub struct TransformerLayer {
    config: TransformerConfig,
    attention: AttentionLayer,
}

impl TransformerLayer {
    pub fn new(config: TransformerConfig) -> Self {
        Self {
            attention: AttentionLayer::new(config.attention.clone()),
            config,
        }
    }

    pub fn forward(&self, x: &Tensor) -> Result<Tensor, TransformerError> {
        // Pipeline fonctionnel pour le forward pass
        self.pre_attention_norm(x)
            .and_then(|normalized| self.compute_attention(&normalized))
            .and_then(|attention_output| self.add_residual(x, &attention_output))
            .and_then(|residual| self.feed_forward(&residual))
    }

    fn pre_attention_norm(&self, x: &Tensor) -> Result<Tensor, TransformerError> {
        // Layer normalization
        Ok(x.layer_norm(None, None, 1e-5))
    }

    fn compute_attention(&self, x: &Tensor) -> Result<Tensor, TransformerError> {
        self.attention
            .compute_scores(x, x)
            .map_err(TransformerError::AttentionError)
    }

    fn add_residual(&self, original: &Tensor, transformed: &Tensor) -> Result<Tensor, TransformerError> {
        Ok(original + transformed)
    }

    fn feed_forward(&self, x: &Tensor) -> Result<Tensor, TransformerError> {
        // Feed forward network avec activation GELU
        Ok(x.gelu())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transformer_config_validation() {
        assert!(TransformerConfig::new(-1, 2, 128).is_err());
        assert!(TransformerConfig::new(64, -2, 128).is_err());
        assert!(TransformerConfig::new(64, 2, -128).is_err());
        assert!(TransformerConfig::new(64, 2, 128).is_ok());
    }

    #[test]
    fn test_transformer_forward() {
        let config = TransformerConfig::new(64, 2, 128).unwrap();
        let transformer = TransformerLayer::new(config);
        let input = Tensor::rand(&[1, 4, 64], (Kind::Float, Device::Cpu));

        let output = transformer.forward(&input);
        assert!(output.is_ok());

        if let Ok(output) = output {
            assert_eq!(output.size(), input.size());
        }
    }

    #[test]
    fn test_transformer_with_dropout() {
        let config = TransformerConfig::new(64, 2, 128)
            .unwrap()
            .with_dropout(0.1)
            .unwrap();
        let transformer = TransformerLayer::new(config);
        let input = Tensor::rand(&[1, 4, 64], (Kind::Float, Device::Cpu));

        assert!(transformer.forward(&input).is_ok());
    }
}
