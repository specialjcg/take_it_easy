// Module stub pour le modèle Transformer
// À compléter selon les besoins du projet

use super::{
    KeyTensor, QueryTensor, TransformerConfig, TransformerError, TransformerLayer, ValueTensor,
};
use tch::{nn, Device, Result as TchResult, Tensor};

pub struct TransformerModel {
    config: TransformerConfig,
    layers: Vec<TransformerLayer>,
}

impl TransformerModel {
    pub fn new<'a>(
        config: TransformerConfig,
        path: &nn::Path<'a>,
    ) -> Result<Self, TransformerError> {
        let mut layers = Vec::with_capacity(config.num_layers as usize);
        for i in 0..config.num_layers {
            layers.push(TransformerLayer::new(
                &config,
                &(path / format!("layer_{}", i)),
            )?);
        }
        Ok(Self { config, layers })
    }

    pub fn forward(&self, input: &Tensor) -> TchResult<Tensor> {
        // Correction : garantir que l'entrée est 3D [batch, seq_len, embed_dim]
        let mut x = match input.dim() {
            2 => input.unsqueeze(1), // [batch, embed_dim] -> [batch, 1, embed_dim]
            3 => input.shallow_clone(),
            _ => {
                return Err(tch::TchError::Kind(format!(
                    "Input tensor must be 2D or 3D, got shape {:?}",
                    input.size()
                )));
            }
        };
        for layer in &self.layers {
            let attended = layer
                .attention
                .forward(
                    QueryTensor(x.shallow_clone()),
                    KeyTensor(x.shallow_clone()),
                    ValueTensor(x.shallow_clone()),
                )
                .map_err(|e| tch::TchError::Kind(e.to_string()))?;
            let normalized = attended.layer_norm::<Tensor>(
                &[self.config.embedding_dim],
                None,
                None,
                1e-5,
                false,
            );
            let ff_output = normalized.apply(&layer.ff1).gelu("none").apply(&layer.ff2);
            x = ff_output;
        }
        if x.dim() == 3 {
            x = x.flatten(1, -1);
        }
        Ok(x)
    }

    // Alias pour compatibilité avec le pipeline d'entraînement
    pub fn predict(&self, input: &Tensor) -> (Tensor, Tensor) {
        let out = self.forward(input).expect("Transformer forward failed");
        assert!(
            out.requires_grad(),
            "La sortie du modèle ne nécessite pas de gradient !"
        );
        // À adapter selon la vraie sortie du modèle
        (
            out.shallow_clone(),
            out.mean(tch::Kind::Float).unsqueeze(-1),
        )
    }
}
