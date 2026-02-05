//! GAT with Jumping Knowledge Networks
//!
//! Jumping Knowledge (JK) aggregates representations from ALL layers instead of
//! just using the final layer. This captures both local (early layers) and
//! global (late layers) structural information.
//!
//! Aggregation modes:
//! - Concat: Concatenate all layer outputs [h_0 || h_1 || ... || h_L]
//! - MaxPool: Element-wise max across layers
//! - Attention: Learned attention weights over layers
//!
//! Reference: Xu et al., "Representation Learning on Graphs with Jumping Knowledge Networks" (ICML 2018)

use crate::neural::tensor_conversion::{GRAPH_EDGES, GRAPH_NODE_COUNT};
use tch::{nn, IndexOp, Kind, Tensor};

/// Jumping Knowledge aggregation mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JKMode {
    Concat,    // Concatenate all layer outputs
    MaxPool,   // Element-wise max across layers
    Attention, // Learned attention weights
}

impl JKMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "concat" => JKMode::Concat,
            "max" | "maxpool" => JKMode::MaxPool,
            "attention" | "attn" => JKMode::Attention,
            _ => JKMode::Concat,
        }
    }
}

/// Single attention head for GAT
#[derive(Debug)]
struct AttentionHead {
    w: nn::Linear,
    a_src: nn::Linear,
    a_dst: nn::Linear,
}

impl AttentionHead {
    fn new(path: &nn::Path, in_dim: i64, out_dim: i64) -> Self {
        let w = nn::linear(path / "w", in_dim, out_dim, Default::default());
        let a_src = nn::linear(
            path / "a_src",
            out_dim,
            1,
            nn::LinearConfig { bias: false, ..Default::default() },
        );
        let a_dst = nn::linear(
            path / "a_dst",
            out_dim,
            1,
            nn::LinearConfig { bias: false, ..Default::default() },
        );
        Self { w, a_src, a_dst }
    }

    fn forward(&self, x: &Tensor, adj_mask: &Tensor, train: bool, dropout: f64) -> Tensor {
        let batch_size = x.size()[0];
        let num_nodes = x.size()[1];

        let h = x.apply(&self.w);
        let attn_src = h.apply(&self.a_src);
        let attn_dst = h.apply(&self.a_dst);
        let attn_scores = (&attn_src + &attn_dst.transpose(1, 2)).leaky_relu();

        let adj_mask_expanded = adj_mask.unsqueeze(0).expand([batch_size, num_nodes, num_nodes], false);
        let attn_scores = attn_scores + &adj_mask_expanded;
        let attn_weights = attn_scores.softmax(-1, Kind::Float);

        let attn_weights = if train && dropout > 0.0 {
            attn_weights.dropout(dropout, train)
        } else {
            attn_weights
        };

        attn_weights.bmm(&h)
    }
}

/// GAT Layer
#[derive(Debug)]
struct GATLayer {
    heads: Vec<AttentionHead>,
    concat: bool,
    out_proj: Option<nn::Linear>,
    ln: nn::LayerNorm,
    residual_proj: Option<nn::Linear>,
}

impl GATLayer {
    fn new(path: &nn::Path, in_dim: i64, out_dim: i64, num_heads: usize, concat: bool) -> Self {
        let head_dim = if concat { out_dim / num_heads as i64 } else { out_dim };

        let mut heads = Vec::with_capacity(num_heads);
        for i in 0..num_heads {
            heads.push(AttentionHead::new(&(path / format!("head_{}", i)), in_dim, head_dim));
        }

        let actual_out_dim = if concat { head_dim * num_heads as i64 } else { head_dim };
        let out_proj = if concat && actual_out_dim != out_dim {
            Some(nn::linear(path / "out_proj", actual_out_dim, out_dim, Default::default()))
        } else {
            None
        };

        let ln = nn::layer_norm(path / "ln", vec![out_dim], nn::LayerNormConfig { eps: 1e-5, ..Default::default() });

        let residual_proj = if in_dim != out_dim {
            Some(nn::linear(path / "residual_proj", in_dim, out_dim, Default::default()))
        } else {
            None
        };

        Self { heads, concat, out_proj, ln, residual_proj }
    }

    fn forward(&self, x: &Tensor, adj_mask: &Tensor, train: bool, dropout: f64) -> Tensor {
        let head_outputs: Vec<Tensor> = self.heads.iter()
            .map(|head| head.forward(x, adj_mask, train, dropout))
            .collect();

        let mut out = if self.concat {
            Tensor::cat(&head_outputs, -1)
        } else {
            let stacked = Tensor::stack(&head_outputs, 0);
            stacked.mean_dim([0].as_slice(), false, Kind::Float)
        };

        if let Some(ref proj) = self.out_proj {
            out = out.apply(proj);
        }

        let residual = match &self.residual_proj {
            Some(proj) => x.apply(proj),
            None => x.shallow_clone(),
        };

        (out + residual).apply(&self.ln).elu()
    }
}

/// Jumping Knowledge aggregation module
struct JKAggregator {
    mode: JKMode,
    attention_weights: Option<nn::Linear>, // For attention mode
    output_proj: Option<nn::Linear>,       // For concat mode to reduce dims
}

impl JKAggregator {
    fn new(path: &nn::Path, mode: JKMode, num_layers: usize, layer_dim: i64, output_dim: i64) -> Self {
        let attention_weights = if mode == JKMode::Attention {
            Some(nn::linear(path / "jk_attn", layer_dim, 1, Default::default()))
        } else {
            None
        };

        let output_proj = if mode == JKMode::Concat {
            let concat_dim = layer_dim * num_layers as i64;
            Some(nn::linear(path / "jk_proj", concat_dim, output_dim, Default::default()))
        } else {
            None
        };

        Self { mode, attention_weights, output_proj }
    }

    /// Aggregate layer representations
    /// layer_outputs: Vec of [batch, nodes, dim] tensors
    /// Returns: [batch, nodes, output_dim]
    fn forward(&self, layer_outputs: &[Tensor], _train: bool) -> Tensor {
        match self.mode {
            JKMode::Concat => {
                // Concatenate all layers: [batch, nodes, dim * num_layers]
                let concat = Tensor::cat(layer_outputs, -1);
                // Project to output dim
                if let Some(ref proj) = self.output_proj {
                    concat.apply(proj)
                } else {
                    concat
                }
            }
            JKMode::MaxPool => {
                // Stack layers: [num_layers, batch, nodes, dim]
                let stacked = Tensor::stack(layer_outputs, 0);
                // Max across layers
                stacked.max_dim(0, false).0
            }
            JKMode::Attention => {
                // Learn attention weights for each layer
                // Stack: [num_layers, batch, nodes, dim]
                let stacked = Tensor::stack(layer_outputs, 0);
                let num_layers = stacked.size()[0];
                let batch_size = stacked.size()[1];
                let num_nodes = stacked.size()[2];
                let dim = stacked.size()[3];

                // Compute attention scores for each layer
                // Reshape to [num_layers * batch * nodes, dim]
                let flat = stacked.view([num_layers * batch_size * num_nodes, dim]);
                let scores = if let Some(ref attn) = self.attention_weights {
                    flat.apply(attn) // [num_layers * batch * nodes, 1]
                } else {
                    Tensor::ones([num_layers * batch_size * num_nodes, 1], (Kind::Float, flat.device()))
                };

                // Reshape scores to [num_layers, batch, nodes, 1]
                let scores = scores.view([num_layers, batch_size, num_nodes, 1]);

                // Softmax across layers
                let weights = scores.softmax(0, Kind::Float); // [num_layers, batch, nodes, 1]

                // Weighted sum
                let weighted = &stacked * &weights; // [num_layers, batch, nodes, dim]
                weighted.sum_dim_intlist([0].as_slice(), false, Kind::Float) // [batch, nodes, dim]
            }
        }
    }
}

/// GAT with Jumping Knowledge
pub struct GATJKNetwork {
    layers: Vec<GATLayer>,
    jk_aggregator: JKAggregator,
    adj_mask: Tensor,
    dropout: f64,
    jk_mode: JKMode,
}

impl GATJKNetwork {
    pub fn new(
        vs: &nn::VarStore,
        input_dim: i64,
        hidden_dims: &[i64],
        num_heads: usize,
        dropout: f64,
        jk_mode: JKMode,
    ) -> Self {
        let p = vs.root();
        let mut layers = Vec::new();

        let dims = if hidden_dims.is_empty() { vec![128, 128] } else { hidden_dims.to_vec() };

        let mut in_dim = input_dim;
        for (idx, &out_dim) in dims.iter().enumerate() {
            let concat = idx < dims.len() - 1;
            let layer = GATLayer::new(
                &(p.clone() / format!("gat_layer_{}", idx)),
                in_dim,
                out_dim,
                num_heads,
                concat,
            );
            layers.push(layer);
            in_dim = out_dim;
        }

        let final_dim = *dims.last().unwrap_or(&128);
        let jk_aggregator = JKAggregator::new(
            &(p.clone() / "jk"),
            jk_mode,
            dims.len(),
            final_dim,
            final_dim,
        );

        let adj_mask = build_adjacency_mask();

        Self { layers, jk_aggregator, adj_mask, dropout, jk_mode }
    }

    pub fn forward(&self, x: &Tensor, train: bool) -> Tensor {
        let mut layer_outputs = Vec::new();
        let mut h = x.shallow_clone();

        for layer in &self.layers {
            h = layer.forward(&h, &self.adj_mask, train, self.dropout);
            layer_outputs.push(h.shallow_clone());
        }

        // Aggregate all layer outputs with Jumping Knowledge
        self.jk_aggregator.forward(&layer_outputs, train)
    }

    pub fn output_dim(&self) -> i64 {
        self.layers.last()
            .map(|l| l.ln.ws.as_ref().map(|ws| ws.size()[0]).unwrap_or(128))
            .unwrap_or(128)
    }

    pub fn mode(&self) -> JKMode {
        self.jk_mode
    }
}

fn build_adjacency_mask() -> Tensor {
    let n = GRAPH_NODE_COUNT as i64;
    let mask = Tensor::full([n, n], f64::NEG_INFINITY, (Kind::Float, tch::Device::Cpu));

    for &(src, dst) in GRAPH_EDGES.iter() {
        let _ = mask.i((src as i64, dst as i64)).fill_(0.0);
        let _ = mask.i((dst as i64, src as i64)).fill_(0.0);
    }

    for i in 0..n {
        let _ = mask.i((i, i)).fill_(0.0);
    }

    mask
}

/// GAT-JK Policy Network
pub struct GATJKPolicyNet {
    encoder: GATJKNetwork,
    head: nn::Linear,
}

impl GATJKPolicyNet {
    pub fn new(
        vs: &nn::VarStore,
        input_dim: i64,
        hidden_dims: &[i64],
        num_heads: usize,
        dropout: f64,
        jk_mode: JKMode,
    ) -> Self {
        let encoder = GATJKNetwork::new(vs, input_dim, hidden_dims, num_heads, dropout, jk_mode);
        let out_dim = encoder.output_dim();
        let head = nn::linear(vs.root() / "policy_head", out_dim, 1, Default::default());

        Self { encoder, head }
    }

    pub fn forward(&self, node_features: &Tensor, train: bool) -> Tensor {
        let h = self.encoder.forward(node_features, train);
        h.apply(&self.head).squeeze_dim(-1)
    }

    pub fn mode(&self) -> JKMode {
        self.encoder.mode()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gat_jk_concat() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let net = GATJKNetwork::new(&vs, 47, &[128, 128], 4, 0.1, JKMode::Concat);

        let x = Tensor::randn([2, 19, 47], (Kind::Float, tch::Device::Cpu));
        let out = net.forward(&x, false);

        assert_eq!(out.size(), vec![2, 19, 128]);
    }

    #[test]
    fn test_gat_jk_maxpool() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let net = GATJKNetwork::new(&vs, 47, &[128, 128], 4, 0.1, JKMode::MaxPool);

        let x = Tensor::randn([2, 19, 47], (Kind::Float, tch::Device::Cpu));
        let out = net.forward(&x, false);

        assert_eq!(out.size(), vec![2, 19, 128]);
    }

    #[test]
    fn test_gat_jk_attention() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let net = GATJKNetwork::new(&vs, 47, &[128, 128], 4, 0.1, JKMode::Attention);

        let x = Tensor::randn([2, 19, 47], (Kind::Float, tch::Device::Cpu));
        let out = net.forward(&x, false);

        assert_eq!(out.size(), vec![2, 19, 128]);
    }

    #[test]
    fn test_gat_jk_policy_net() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let policy = GATJKPolicyNet::new(&vs, 47, &[128, 128], 4, 0.1, JKMode::Concat);

        let x = Tensor::randn([2, 19, 47], (Kind::Float, tch::Device::Cpu));
        let logits = policy.forward(&x, false);

        assert_eq!(logits.size(), vec![2, 19]);
    }
}
