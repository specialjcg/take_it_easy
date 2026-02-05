//! Graph Attention Network (GAT) for hexagonal Take It Easy board
//!
//! Advantages over standard GNN:
//! - Learned attention weights instead of uniform neighbor averaging
//! - Multi-head attention for stability
//! - Reduces over-smoothing by learning which neighbors matter
//!
//! Reference: Veličković et al., "Graph Attention Networks" (ICLR 2018)

use crate::neural::tensor_conversion::{GRAPH_EDGES, GRAPH_NODE_COUNT};
use tch::{nn, IndexOp, Kind, Tensor};

/// Single attention head
#[derive(Debug)]
pub struct AttentionHead {
    w: nn::Linear,           // Feature transformation
    a_src: nn::Linear,       // Attention for source node
    a_dst: nn::Linear,       // Attention for destination node
}

impl AttentionHead {
    pub fn new(path: &nn::Path, in_dim: i64, out_dim: i64) -> Self {
        let w = nn::linear(path / "w", in_dim, out_dim, Default::default());
        let a_src = nn::linear(
            path / "a_src",
            out_dim,
            1,
            nn::LinearConfig {
                bias: false,
                ..Default::default()
            },
        );
        let a_dst = nn::linear(
            path / "a_dst",
            out_dim,
            1,
            nn::LinearConfig {
                bias: false,
                ..Default::default()
            },
        );

        Self {
            w,
            a_src,
            a_dst,
        }
    }

    /// Forward pass for single attention head
    /// x: [batch, nodes, in_features]
    /// adj_mask: [nodes, nodes] - 1 where edge exists, -inf where not
    /// Returns: [batch, nodes, out_features]
    pub fn forward(&self, x: &Tensor, adj_mask: &Tensor, train: bool, dropout: f64) -> Tensor {
        let batch_size = x.size()[0];
        let num_nodes = x.size()[1];

        // Transform features: [batch, nodes, out_dim]
        let h = x.apply(&self.w);

        // Compute attention scores
        // e_ij = LeakyReLU(a_src * h_i + a_dst * h_j)
        let attn_src = h.apply(&self.a_src); // [batch, nodes, 1]
        let attn_dst = h.apply(&self.a_dst); // [batch, nodes, 1]

        // Broadcast to create attention matrix
        // attn_src: [batch, nodes, 1] -> [batch, nodes, nodes] (broadcast over last dim)
        // attn_dst: [batch, 1, nodes] (transpose) -> [batch, nodes, nodes] (broadcast over middle dim)
        let attn_scores = &attn_src + &attn_dst.transpose(1, 2); // [batch, nodes, nodes]

        // Apply LeakyReLU (negative_slope = 0.2)
        let attn_scores = attn_scores.leaky_relu();

        // Mask non-edges with -inf (so softmax gives 0)
        let adj_mask_expanded = adj_mask.unsqueeze(0).expand([batch_size, num_nodes, num_nodes], false);
        let attn_scores = attn_scores + &adj_mask_expanded;

        // Softmax over neighbors
        let attn_weights = attn_scores.softmax(-1, Kind::Float); // [batch, nodes, nodes]

        // Apply dropout to attention weights during training
        let attn_weights = if train && dropout > 0.0 {
            attn_weights.dropout(dropout, train)
        } else {
            attn_weights
        };

        // Aggregate neighbor features weighted by attention
        // [batch, nodes, nodes] @ [batch, nodes, out_dim] -> [batch, nodes, out_dim]
        attn_weights.bmm(&h)
    }
}

/// Multi-head Graph Attention Layer
#[derive(Debug)]
pub struct GATLayer {
    heads: Vec<AttentionHead>,
    concat: bool,                    // If true, concat heads; if false, average
    out_proj: Option<nn::Linear>,    // Optional projection after concat
    ln: nn::LayerNorm,
    residual_proj: Option<nn::Linear>, // For dimension mismatch
}

impl GATLayer {
    pub fn new(
        path: &nn::Path,
        in_dim: i64,
        out_dim: i64,
        num_heads: usize,
        concat: bool,
    ) -> Self {
        let head_dim = if concat {
            out_dim / num_heads as i64
        } else {
            out_dim
        };

        let mut heads = Vec::with_capacity(num_heads);
        for i in 0..num_heads {
            heads.push(AttentionHead::new(&(path / format!("head_{}", i)), in_dim, head_dim));
        }

        let actual_out_dim = if concat {
            head_dim * num_heads as i64
        } else {
            head_dim
        };

        // Optional projection to match desired output dim
        let out_proj = if concat && actual_out_dim != out_dim {
            Some(nn::linear(path / "out_proj", actual_out_dim, out_dim, Default::default()))
        } else {
            None
        };

        let ln = nn::layer_norm(
            path / "ln",
            vec![out_dim],
            nn::LayerNormConfig {
                eps: 1e-5,
                ..Default::default()
            },
        );

        // Residual projection if dimensions don't match
        let residual_proj = if in_dim != out_dim {
            Some(nn::linear(path / "residual_proj", in_dim, out_dim, Default::default()))
        } else {
            None
        };

        Self {
            heads,
            concat,
            out_proj,
            ln,
            residual_proj,
        }
    }

    pub fn forward(&self, x: &Tensor, adj_mask: &Tensor, train: bool, dropout: f64) -> Tensor {
        // Compute all attention heads
        let head_outputs: Vec<Tensor> = self
            .heads
            .iter()
            .map(|head| head.forward(x, adj_mask, train, dropout))
            .collect();

        // Combine heads
        let mut out = if self.concat {
            // Concatenate along feature dimension
            Tensor::cat(&head_outputs, -1)
        } else {
            // Average heads
            let stacked = Tensor::stack(&head_outputs, 0);
            stacked.mean_dim([0].as_slice(), false, Kind::Float)
        };

        // Optional projection
        if let Some(ref proj) = self.out_proj {
            out = out.apply(proj);
        }

        // Residual connection
        let residual = match &self.residual_proj {
            Some(proj) => x.apply(proj),
            None => x.shallow_clone(),
        };

        // LayerNorm + Residual
        out = (out + residual).apply(&self.ln);

        // ELU activation (common for GAT, smoother than ReLU)
        out.elu()
    }
}

/// Complete Graph Attention Network
pub struct GraphAttentionNetwork {
    layers: Vec<GATLayer>,
    adj_mask: Tensor,
    dropout: f64,
}

impl GraphAttentionNetwork {
    /// Create a new GAT
    ///
    /// # Arguments
    /// * `vs` - Variable store
    /// * `input_dim` - Input feature dimension (8 for current encoding)
    /// * `hidden_dims` - Hidden layer dimensions (e.g., [64, 64])
    /// * `num_heads` - Number of attention heads per layer
    /// * `dropout` - Dropout rate
    pub fn new(
        vs: &nn::VarStore,
        input_dim: i64,
        hidden_dims: &[i64],
        num_heads: usize,
        dropout: f64,
    ) -> Self {
        let p = vs.root();
        let mut layers = Vec::new();

        let dims = if hidden_dims.is_empty() {
            vec![64, 64] // Default
        } else {
            hidden_dims.to_vec()
        };

        let mut in_dim = input_dim;
        for (idx, &out_dim) in dims.iter().enumerate() {
            // Use concat for all layers except last (average for last)
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

        let adj_mask = build_adjacency_mask();

        Self {
            layers,
            adj_mask,
            dropout,
        }
    }

    pub fn forward(&self, x: &Tensor, train: bool) -> Tensor {
        let mut h = x.shallow_clone();
        for layer in &self.layers {
            h = layer.forward(&h, &self.adj_mask, train, self.dropout);
        }
        h
    }

    pub fn output_dim(&self) -> i64 {
        self.layers.last().map(|l| {
            // Get output dim from layer norm weights
            l.ln.ws.as_ref().map(|ws| ws.size()[0]).unwrap_or(64)
        }).unwrap_or(64)
    }
}

/// Build adjacency mask for attention
/// Returns matrix where edges have 0 and non-edges have -inf
fn build_adjacency_mask() -> Tensor {
    let n = GRAPH_NODE_COUNT as i64;

    // Start with -inf everywhere
    let mask = Tensor::full([n, n], f64::NEG_INFINITY, (Kind::Float, tch::Device::Cpu));

    // Set 0 for existing edges (including self-loops)
    for &(src, dst) in GRAPH_EDGES.iter() {
        let _ = mask.i((src as i64, dst as i64)).fill_(0.0);
        let _ = mask.i((dst as i64, src as i64)).fill_(0.0);
    }

    // Self-loops
    for i in 0..n {
        let _ = mask.i((i, i)).fill_(0.0);
    }

    mask
}

/// GAT-based Policy Network for Take It Easy
pub struct GATPolicyNet {
    encoder: GraphAttentionNetwork,
    head: nn::Linear,
}

impl GATPolicyNet {
    pub fn new(
        vs: &nn::VarStore,
        input_dim: i64,
        hidden_dims: &[i64],
        num_heads: usize,
        dropout: f64,
    ) -> Self {
        let encoder = GraphAttentionNetwork::new(vs, input_dim, hidden_dims, num_heads, dropout);
        let out_dim = encoder.output_dim();
        let head = nn::linear(vs.root() / "policy_head", out_dim, 1, Default::default());

        Self { encoder, head }
    }

    /// Forward pass
    /// node_features: [batch, 19, input_dim]
    /// Returns: [batch, 19] logits (apply softmax externally)
    pub fn forward(&self, node_features: &Tensor, train: bool) -> Tensor {
        let h = self.encoder.forward(node_features, train);
        h.apply(&self.head).squeeze_dim(-1)
    }

    /// Get attention weights for visualization/debugging
    /// Returns attention weights from last layer, first head
    pub fn get_attention_weights(&self, node_features: &Tensor) -> Tensor {
        // This would require storing attention weights during forward pass
        // For now, just do a forward pass and return dummy
        let _ = self.encoder.forward(node_features, false);
        Tensor::zeros([19, 19], (Kind::Float, tch::Device::Cpu))
    }
}

/// GAT-based Value Network for Take It Easy
pub struct GATValueNet {
    encoder: GraphAttentionNetwork,
    head: nn::Sequential,
}

impl GATValueNet {
    pub fn new(
        vs: &nn::VarStore,
        input_dim: i64,
        hidden_dims: &[i64],
        num_heads: usize,
        dropout: f64,
    ) -> Self {
        let encoder = GraphAttentionNetwork::new(vs, input_dim, hidden_dims, num_heads, dropout);
        let out_dim = encoder.output_dim();

        // Two-layer value head for more expressiveness
        let head = nn::seq()
            .add(nn::linear(vs.root() / "value_fc1", out_dim, 64, Default::default()))
            .add_fn(|x| x.relu())
            .add(nn::linear(vs.root() / "value_fc2", 64, 1, Default::default()));

        Self { encoder, head }
    }

    /// Forward pass
    /// node_features: [batch, 19, input_dim]
    /// Returns: [batch, 1] value in [-1, 1]
    pub fn forward(&self, node_features: &Tensor, train: bool) -> Tensor {
        let h = self.encoder.forward(node_features, train);

        // Global pooling: attention-weighted mean instead of simple mean
        // For now, use mean pooling (can be improved with learned pooling)
        let pooled = h.mean_dim([1].as_slice(), false, Kind::Float);

        pooled.apply(&self.head).tanh()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gat_forward() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let gat = GraphAttentionNetwork::new(&vs, 8, &[64, 64], 4, 0.1);

        // Batch of 2, 19 nodes, 8 features
        let x = Tensor::randn([2, 19, 8], (Kind::Float, tch::Device::Cpu));
        let out = gat.forward(&x, false);

        assert_eq!(out.size(), vec![2, 19, 64]);
    }

    #[test]
    fn test_gat_policy_net() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let policy = GATPolicyNet::new(&vs, 8, &[64, 64], 4, 0.1);

        let x = Tensor::randn([2, 19, 8], (Kind::Float, tch::Device::Cpu));
        let logits = policy.forward(&x, false);

        assert_eq!(logits.size(), vec![2, 19]);
    }

    #[test]
    fn test_gat_value_net() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let value = GATValueNet::new(&vs, 8, &[64, 64], 4, 0.1);

        let x = Tensor::randn([2, 19, 8], (Kind::Float, tch::Device::Cpu));
        let v = value.forward(&x, false);

        assert_eq!(v.size(), vec![2, 1]);
        // Value should be in [-1, 1] due to tanh
        let v_vec: Vec<f64> = v.view([-1]).try_into().unwrap();
        for val in v_vec {
            assert!(val >= -1.0 && val <= 1.0);
        }
    }

    #[test]
    fn test_adjacency_mask() {
        let mask = build_adjacency_mask();
        assert_eq!(mask.size(), vec![19, 19]);

        // Check self-loops are 0
        for i in 0..19 {
            let val: f64 = mask.i((i, i)).try_into().unwrap();
            assert_eq!(val, 0.0);
        }

        // Check known edge (0, 1) is 0
        let val: f64 = mask.i((0, 1)).try_into().unwrap();
        assert_eq!(val, 0.0);

        // Check non-edge (0, 18) is -inf
        let val: f64 = mask.i((0, 18)).try_into().unwrap();
        assert!(val.is_infinite() && val < 0.0);
    }
}
