use std::result::Result;
use tch::nn;
use tch::{Device, Kind, Tensor};

#[derive(Debug)]
pub struct QueryTensor(pub Tensor);

#[derive(Debug)]
pub struct KeyTensor(pub Tensor);

#[derive(Debug)]
pub struct ValueTensor(pub Tensor);

impl Clone for QueryTensor {
    fn clone(&self) -> Self {
        Self(self.0.shallow_clone())
    }
}

impl Clone for KeyTensor {
    fn clone(&self) -> Self {
        Self(self.0.shallow_clone())
    }
}

impl Clone for ValueTensor {
    fn clone(&self) -> Self {
        Self(self.0.shallow_clone())
    }
}

#[derive(Clone, Debug)]
pub struct AttentionError {
    kind: AttentionErrorKind,
    message: String,
}

#[derive(Clone, Debug)]
pub enum AttentionErrorKind {
    InvalidDimension,
    TensorError,
    ShapeMismatch,
    ComputationError,
    DeviceError,
}

impl std::fmt::Display for AttentionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl AttentionError {
    pub fn new(kind: AttentionErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

// Types immuables pour la configuration
#[derive(Clone, Debug)]
pub struct AttentionDim(i64);
#[derive(Clone, Debug)]
pub struct NumHeads(i64);
#[derive(Clone, Debug)]
pub struct DropoutRate(f64);

impl AttentionDim {
    pub fn new(dim: i64) -> Result<Self, AttentionError> {
        if dim <= 0 {
            return Err(AttentionError::new(
                AttentionErrorKind::InvalidDimension,
                "Dimension must be positive",
            ));
        }
        Ok(Self(dim))
    }

    pub fn value(&self) -> i64 {
        self.0
    }
}

impl NumHeads {
    pub fn new(heads: i64) -> Result<Self, AttentionError> {
        if heads <= 0 {
            return Err(AttentionError::new(
                AttentionErrorKind::InvalidDimension,
                "Number of heads must be positive",
            ));
        }
        Ok(Self(heads))
    }

    pub fn value(&self) -> i64 {
        self.0
    }
}

impl DropoutRate {
    pub fn new(rate: f64) -> Result<Self, AttentionError> {
        if !(0.0..=1.0).contains(&rate) {
            return Err(AttentionError::new(
                AttentionErrorKind::InvalidDimension,
                "Dropout rate must be between 0 and 1",
            ));
        }
        Ok(Self(rate))
    }

    pub fn value(&self) -> f64 {
        self.0
    }
}

#[derive(Clone, Debug)]
pub struct AttentionConfig {
    dim: AttentionDim,
    num_heads: NumHeads,
    dropout: Option<DropoutRate>,
}

impl AttentionConfig {
    pub fn new(dim: i64, num_heads: i64) -> Result<Self, AttentionError> {
        let dim = AttentionDim::new(dim)?;
        let heads = NumHeads::new(num_heads)?;

        if dim.value() % heads.value() != 0 {
            return Err(AttentionError::new(
                AttentionErrorKind::InvalidDimension,
                "Dimension must be divisible by num_heads",
            ));
        }

        Ok(Self {
            dim,
            num_heads: heads,
            dropout: None,
        })
    }

    pub fn with_dropout(self, rate: f64) -> Result<Self, AttentionError> {
        Ok(Self {
            dropout: Some(DropoutRate::new(rate)?),
            ..self
        })
    }
}

pub struct AttentionLayer {
    pub config: AttentionConfig,
    pub linear_q: nn::Linear,
    pub linear_k: nn::Linear,
    pub linear_v: nn::Linear,
    pub linear_out: nn::Linear,
}

impl AttentionLayer {
    pub fn new<'a>(config: AttentionConfig, path: &nn::Path<'a>) -> Self {
        let embed_dim = config.dim.value();
        let linear_q = nn::linear(path / "q_proj", embed_dim, embed_dim, Default::default());
        let linear_k = nn::linear(path / "k_proj", embed_dim, embed_dim, Default::default());
        let linear_v = nn::linear(path / "v_proj", embed_dim, embed_dim, Default::default());
        let linear_out = nn::linear(path / "out_proj", embed_dim, embed_dim, Default::default());
        Self {
            config,
            linear_q,
            linear_k,
            linear_v,
            linear_out,
        }
    }

    pub fn compute_scores(&self, query: &Tensor, key: &Tensor) -> Tensor {
        // Calcul minimaliste des scores d'attention (produit scalaire)
        query.matmul(&key.transpose(-2, -1))
    }
}

impl AttentionLayer {
    pub fn forward(
        &self,
        query: QueryTensor,
        key: KeyTensor,
        value: ValueTensor,
    ) -> Result<Tensor, AttentionError> {
        self.validate_shapes(&query, &key, &value)
            .map(|_| self.compute_attention(query, key, value))?
    }

    fn validate_shapes(
        &self,
        query: &QueryTensor,
        key: &KeyTensor,
        value: &ValueTensor,
    ) -> Result<(), AttentionError> {
        let q_size = query.0.size();
        let k_size = key.0.size();
        let v_size = value.0.size();

        if q_size.len() != 3 || k_size.len() != 3 || v_size.len() != 3 {
            return Err(AttentionError::new(
                AttentionErrorKind::ShapeMismatch,
                "All inputs must be 3D tensors",
            ));
        }
        if q_size[2] != k_size[2] || k_size[2] != v_size[2] {
            return Err(AttentionError::new(
                AttentionErrorKind::ShapeMismatch,
                "All dimensions must match",
            ));
        }
        Ok(())
    }

    fn compute_attention(
        &self,
        query: QueryTensor,
        key: KeyTensor,
        value: ValueTensor,
    ) -> Result<Tensor, AttentionError> {
        let scores = scale_dot_product(&query, &key, &self.config.dim)?;
        let mask = create_attention_mask(query.0.size()[1]);
        let masked_scores = apply_mask(scores, &mask);
        let attention_weights = masked_scores.softmax(-1, query.0.kind());

        Ok(attention_weights.matmul(&value.0))
    }

    pub fn get_attention_weights(
        &self,
        query: QueryTensor,
        key: KeyTensor,
        value: ValueTensor,
    ) -> Result<Tensor, AttentionError> {
        self.validate_shapes(&query, &key, &value)
            .map(|_| self.compute_raw_attention_weights(query, key))?
    }

    fn compute_raw_attention_weights(
        &self,
        query: QueryTensor,
        key: KeyTensor,
    ) -> Result<Tensor, AttentionError> {
        let scale = (self.config.dim.value() as f64).powf(-0.5);
        let attention_scores = query.0.matmul(&key.0.transpose(-2, -1)) * scale;
        Ok(attention_scores.softmax(-1, query.0.kind()))
    }
}

// Fonctions utilitaires
fn create_attention_mask(size: i64) -> Tensor {
    Tensor::ones(&[size, size], (Kind::Float, Device::Cpu))
}

fn scale_dot_product(
    query: &QueryTensor,
    key: &KeyTensor,
    dim: &AttentionDim,
) -> Result<Tensor, AttentionError> {
    let scale = (dim.value() as f64).powf(-0.5);
    Ok(query.0.matmul(&key.0.transpose(-2, -1)) * scale)
}

fn apply_mask(scores: Tensor, mask: &Tensor) -> Tensor {
    scores * mask
}

// Traits pour la composition fonctionnelle
pub trait AttentionTransform {
    fn transform(&self, input: Tensor) -> Result<Tensor, AttentionError>;
}

impl AttentionTransform for AttentionLayer {
    fn transform(&self, input: Tensor) -> Result<Tensor, AttentionError> {
        let query = QueryTensor(input.shallow_clone());
        let key = KeyTensor(input.shallow_clone());
        let value = ValueTensor(input);
        self.forward(query, key, value)
    }
}

pub trait ComposableAttention: AttentionTransform {
    fn compose<T: AttentionTransform>(self, next: T) -> AttentionComposition<Self, T>
    where
        Self: Sized,
    {
        AttentionComposition {
            first: self,
            second: next,
        }
    }
}

pub struct AttentionComposition<T1, T2> {
    first: T1,
    second: T2,
}

impl<T1, T2> AttentionTransform for AttentionComposition<T1, T2>
where
    T1: AttentionTransform,
    T2: AttentionTransform,
{
    fn transform(&self, input: Tensor) -> Result<Tensor, AttentionError> {
        self.first
            .transform(input)
            .and_then(|output| self.second.transform(output))
    }
}

impl ComposableAttention for AttentionLayer {}

#[cfg(test)]
mod tests {
    use super::*;
    use tch::nn;

    fn create_random_tensor(batch: i64, seq_len: i64, dim: i64) -> Tensor {
        Tensor::rand(&[batch, seq_len, dim], (Kind::Float, Device::Cpu))
    }

    #[test]
    fn test_attention_config_validation() {
        assert!(AttentionConfig::new(-1, 2).is_err());
        assert!(AttentionConfig::new(63, 2).is_err());
        assert!(AttentionConfig::new(64, 2).is_ok());
    }

    #[test]
    fn test_attention_forward() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let config = AttentionConfig::new(64, 2).unwrap();
        let attention = AttentionLayer::new(config, &vs.root());

        let query = QueryTensor(create_random_tensor(1, 4, 64));
        let key = KeyTensor(create_random_tensor(1, 4, 64));
        let value = ValueTensor(create_random_tensor(1, 4, 64));

        let output = attention.forward(query, key, value);
        assert!(output.is_ok());

        if let Ok(output) = output {
            assert_eq!(output.size(), &[1, 4, 64]);
        }
    }

    #[test]
    fn test_attention_masking() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let config = AttentionConfig::new(64, 2).unwrap();
        let attention = AttentionLayer::new(config, &vs.root());

        let query = QueryTensor(create_random_tensor(1, 4, 64));
        let key = KeyTensor(create_random_tensor(1, 4, 64));
        let value = ValueTensor(create_random_tensor(1, 4, 64));

        let output = attention.forward(query, key, value);
        assert!(output.is_ok());

        if let Ok(output) = output {
            assert_eq!(output.size(), &[1, 4, 64]);
        }
    }

    #[test]
    fn test_invalid_shapes() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let config = AttentionConfig::new(64, 2).unwrap();
        let attention = AttentionLayer::new(config, &vs.root());

        let query = QueryTensor(create_random_tensor(1, 4, 64));
        let key = KeyTensor(create_random_tensor(1, 4, 32));
        let value = ValueTensor(create_random_tensor(1, 4, 64));

        assert!(attention.forward(query, key, value).is_err());
    }

    #[test]
    fn test_attention_composition() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let config1 = AttentionConfig::new(64, 2).unwrap();
        let config2 = AttentionConfig::new(64, 2).unwrap();
        let attention1 = AttentionLayer::new(config1, &vs.root());
        let attention2 = AttentionLayer::new(config2, &vs.root());

        let composed = attention1.compose(attention2);

        let input = create_random_tensor(1, 4, 64);
        let output = composed.transform(input);
        assert!(output.is_ok());

        if let Ok(output) = output {
            assert_eq!(output.size(), &[1, 4, 64]);
        }
    }

    #[test]
    fn test_attention_weights() {
        let vs = tch::nn::VarStore::new(tch::Device::Cpu);
        let config = AttentionConfig::new(64, 2).unwrap();
        let attention = AttentionLayer::new(config, &vs.root());

        let query = QueryTensor(create_random_tensor(1, 4, 64));
        let key = KeyTensor(create_random_tensor(1, 4, 64));
        let value = ValueTensor(create_random_tensor(1, 4, 64));

        let weights = attention.get_attention_weights(query, key, value);
        assert!(weights.is_ok());

        if let Ok(weights) = weights {
            // Vérification des dimensions
            assert_eq!(weights.size(), &[1, 4, 4]);

            // Vérification que la somme des poids par ligne est proche de 1
            let sum = weights.sum_dim_intlist(&[-1i64][..], false, Kind::Float);
            let ones = Tensor::ones(&[1, 4], (Kind::Float, Device::Cpu));
            assert!(sum.allclose(&ones, 1e-5, 1e-8, false));
        }
    }

    #[test]
    fn test_attention_pattern() {
        let vs = tch::nn::VarStore::new(tch::Device::Cpu);
        let config = AttentionConfig::new(64, 2).unwrap();
        let attention = AttentionLayer::new(config, &vs.root());

        // Créer des tenseurs avec un pattern d'attention clair
        let query = QueryTensor(Tensor::ones(&[1, 4, 64], (Kind::Float, Device::Cpu)));
        let key = KeyTensor(Tensor::ones(&[1, 4, 64], (Kind::Float, Device::Cpu)));
        let value = ValueTensor(Tensor::ones(&[1, 4, 64], (Kind::Float, Device::Cpu)));

        let weights = attention.get_attention_weights(query, key, value).unwrap();

        // Les poids devraient être uniformes dans ce cas
        let expected = Tensor::ones(&[1, 4, 4], (Kind::Float, Device::Cpu)) / 4.0;
        assert!(weights.allclose(&expected, 1e-5, 1e-8, false));
    }
}
