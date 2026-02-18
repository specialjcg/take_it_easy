//! Graph Transformer for hexagonal Take It Easy board
//!
//! Unlike GAT which only attends to neighboring nodes, Graph Transformer
//! uses full self-attention between ALL 19 nodes. This allows the model to:
//! - Capture long-range dependencies (e.g., opposite corners sharing diagonal lines)
//! - Learn which positions are strategically related regardless of adjacency
//! - Potentially better model the scoring structure where non-adjacent positions matter
//!
//! Key differences from GAT:
//! - Full attention (no adjacency mask)
//! - Positional encoding to distinguish hex positions
//! - Standard transformer architecture with LayerNorm and residual connections

use tch::{nn, Kind, Tensor};

const NODE_COUNT: usize = 19;

/// Learnable positional encoding for each hex position
#[derive(Debug)]
pub struct PositionalEncoding {
    pos_embed: Tensor, // [19, embed_dim]
}

impl PositionalEncoding {
    pub fn new(path: &nn::Path, embed_dim: i64) -> Self {
        // Learnable position embeddings
        let pos_embed = path.var("pos_embed", &[NODE_COUNT as i64, embed_dim], nn::Init::Randn { mean: 0.0, stdev: 0.02 });
        Self { pos_embed }
    }

    /// Add positional encoding to node features
    /// x: [batch, 19, embed_dim]
    /// Returns: [batch, 19, embed_dim]
    pub fn forward(&self, x: &Tensor) -> Tensor {
        x + &self.pos_embed.unsqueeze(0)
    }
}

/// Multi-head self-attention (full attention, no masking)
#[derive(Debug)]
pub struct MultiHeadAttention {
    q_proj: nn::Linear,
    k_proj: nn::Linear,
    v_proj: nn::Linear,
    out_proj: nn::Linear,
    num_heads: i64,
    head_dim: i64,
    scale: f64,
}

impl MultiHeadAttention {
    pub fn new(path: &nn::Path, embed_dim: i64, num_heads: i64) -> Self {
        let head_dim = embed_dim / num_heads;
        let scale = (head_dim as f64).sqrt();

        let q_proj = nn::linear(path / "q_proj", embed_dim, embed_dim, Default::default());
        let k_proj = nn::linear(path / "k_proj", embed_dim, embed_dim, Default::default());
        let v_proj = nn::linear(path / "v_proj", embed_dim, embed_dim, Default::default());
        let out_proj = nn::linear(path / "out_proj", embed_dim, embed_dim, Default::default());

        Self {
            q_proj,
            k_proj,
            v_proj,
            out_proj,
            num_heads,
            head_dim,
            scale,
        }
    }

    /// Forward pass with full self-attention
    /// x: [batch, 19, embed_dim]
    /// Returns: [batch, 19, embed_dim]
    pub fn forward(&self, x: &Tensor, train: bool, dropout: f64) -> Tensor {
        let batch_size = x.size()[0];
        let seq_len = x.size()[1];

        // Project to Q, K, V
        let q = x.apply(&self.q_proj); // [batch, 19, embed_dim]
        let k = x.apply(&self.k_proj);
        let v = x.apply(&self.v_proj);

        // Reshape for multi-head attention: [batch, heads, seq, head_dim]
        let q = q.view([batch_size, seq_len, self.num_heads, self.head_dim])
            .permute([0, 2, 1, 3]);
        let k = k.view([batch_size, seq_len, self.num_heads, self.head_dim])
            .permute([0, 2, 1, 3]);
        let v = v.view([batch_size, seq_len, self.num_heads, self.head_dim])
            .permute([0, 2, 1, 3]);

        // Compute attention scores: [batch, heads, seq, seq]
        let attn_scores = q.matmul(&k.transpose(-2, -1)) / self.scale;

        // Softmax (full attention - no masking)
        let attn_weights = attn_scores.softmax(-1, Kind::Float);

        // Apply dropout to attention weights
        let attn_weights = if train && dropout > 0.0 {
            attn_weights.dropout(dropout, train)
        } else {
            attn_weights
        };

        // Apply attention to values: [batch, heads, seq, head_dim]
        let out = attn_weights.matmul(&v);

        // Reshape back: [batch, seq, embed_dim]
        let out = out.permute([0, 2, 1, 3]).contiguous()
            .view([batch_size, seq_len, self.num_heads * self.head_dim]);

        // Output projection
        out.apply(&self.out_proj)
    }
}

/// Feed-forward network (MLP)
#[derive(Debug)]
pub struct FeedForward {
    fc1: nn::Linear,
    fc2: nn::Linear,
}

impl FeedForward {
    pub fn new(path: &nn::Path, embed_dim: i64, ff_dim: i64) -> Self {
        let fc1 = nn::linear(path / "fc1", embed_dim, ff_dim, Default::default());
        let fc2 = nn::linear(path / "fc2", ff_dim, embed_dim, Default::default());
        Self { fc1, fc2 }
    }

    pub fn forward(&self, x: &Tensor, train: bool, dropout: f64) -> Tensor {
        let h = x.apply(&self.fc1).gelu("none");
        let h = if train && dropout > 0.0 {
            h.dropout(dropout, train)
        } else {
            h
        };
        h.apply(&self.fc2)
    }
}

/// Single Transformer encoder layer
#[derive(Debug)]
pub struct TransformerLayer {
    attn: MultiHeadAttention,
    ff: FeedForward,
    ln1: nn::LayerNorm,
    ln2: nn::LayerNorm,
}

impl TransformerLayer {
    pub fn new(path: &nn::Path, embed_dim: i64, num_heads: i64, ff_dim: i64) -> Self {
        let attn = MultiHeadAttention::new(&(path / "attn"), embed_dim, num_heads);
        let ff = FeedForward::new(&(path / "ff"), embed_dim, ff_dim);

        let ln1 = nn::layer_norm(
            path / "ln1",
            vec![embed_dim],
            nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
        );
        let ln2 = nn::layer_norm(
            path / "ln2",
            vec![embed_dim],
            nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
        );

        Self { attn, ff, ln1, ln2 }
    }

    /// Pre-LN Transformer layer (more stable training)
    pub fn forward(&self, x: &Tensor, train: bool, dropout: f64) -> Tensor {
        // Self-attention with residual
        let normed = x.apply(&self.ln1);
        let attn_out = self.attn.forward(&normed, train, dropout);
        let attn_out = if train && dropout > 0.0 {
            attn_out.dropout(dropout, train)
        } else {
            attn_out
        };
        let x = x + attn_out;

        // Feed-forward with residual
        let normed = x.apply(&self.ln2);
        let ff_out = self.ff.forward(&normed, train, dropout);
        let ff_out = if train && dropout > 0.0 {
            ff_out.dropout(dropout, train)
        } else {
            ff_out
        };
        x + ff_out
    }
}

/// Graph Transformer network
pub struct GraphTransformer {
    input_proj: nn::Linear,
    pos_encoding: PositionalEncoding,
    layers: Vec<TransformerLayer>,
    final_ln: nn::LayerNorm,
    dropout: f64,
}

impl GraphTransformer {
    pub fn new(
        vs: &nn::VarStore,
        input_dim: i64,
        embed_dim: i64,
        num_layers: usize,
        num_heads: i64,
        ff_dim: i64,
        dropout: f64,
    ) -> Self {
        let p = vs.root();

        // Project input features to embedding dimension
        let input_proj = nn::linear(p.clone() / "input_proj", input_dim, embed_dim, Default::default());

        // Positional encoding
        let pos_encoding = PositionalEncoding::new(&(p.clone() / "pos_enc"), embed_dim);

        // Transformer layers
        let mut layers = Vec::with_capacity(num_layers);
        for i in 0..num_layers {
            layers.push(TransformerLayer::new(
                &(p.clone() / format!("layer_{}", i)),
                embed_dim,
                num_heads,
                ff_dim,
            ));
        }

        // Final layer norm
        let final_ln = nn::layer_norm(
            p / "final_ln",
            vec![embed_dim],
            nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
        );

        Self {
            input_proj,
            pos_encoding,
            layers,
            final_ln,
            dropout,
        }
    }

    /// Forward pass
    /// x: [batch, 19, input_dim]
    /// Returns: [batch, 19, embed_dim]
    pub fn forward(&self, x: &Tensor, train: bool) -> Tensor {
        // Project to embedding dimension
        let mut h = x.apply(&self.input_proj);

        // Add positional encoding
        h = self.pos_encoding.forward(&h);

        // Apply dropout after embedding
        if train && self.dropout > 0.0 {
            h = h.dropout(self.dropout, train);
        }

        // Pass through transformer layers
        for layer in &self.layers {
            h = layer.forward(&h, train, self.dropout);
        }

        // Final layer norm
        h.apply(&self.final_ln)
    }

    pub fn output_dim(&self) -> i64 {
        self.final_ln.ws.as_ref().map(|ws| ws.size()[0]).unwrap_or(128)
    }
}

/// Graph Transformer Policy Network
pub struct GraphTransformerPolicyNet {
    transformer: GraphTransformer,
    policy_head: nn::Linear,
}

impl GraphTransformerPolicyNet {
    pub fn new(
        vs: &nn::VarStore,
        input_dim: i64,
        embed_dim: i64,
        num_layers: usize,
        num_heads: i64,
        dropout: f64,
    ) -> Self {
        let ff_dim = embed_dim * 4; // Standard transformer uses 4x expansion

        let transformer = GraphTransformer::new(
            vs,
            input_dim,
            embed_dim,
            num_layers,
            num_heads,
            ff_dim,
            dropout,
        );

        let out_dim = transformer.output_dim();
        let policy_head = nn::linear(vs.root() / "policy_head", out_dim, 1, Default::default());

        Self {
            transformer,
            policy_head,
        }
    }

    /// Forward pass
    /// node_features: [batch, 19, input_dim]
    /// Returns: [batch, 19] logits
    pub fn forward(&self, node_features: &Tensor, train: bool) -> Tensor {
        let h = self.transformer.forward(node_features, train);
        h.apply(&self.policy_head).squeeze_dim(-1)
    }
}

/// Graph Transformer Value Network
///
/// Shares the same backbone as GraphTransformerPolicyNet but uses mean pooling
/// over nodes followed by an MLP to produce a single scalar value in [-1, 1].
pub struct GraphTransformerValueNet {
    transformer: GraphTransformer,
    value_head: nn::Sequential,
}

impl GraphTransformerValueNet {
    pub fn new(
        vs: &nn::VarStore,
        input_dim: i64,
        embed_dim: i64,
        num_layers: usize,
        num_heads: i64,
        dropout: f64,
    ) -> Self {
        let ff_dim = embed_dim * 4;

        let transformer = GraphTransformer::new(
            vs,
            input_dim,
            embed_dim,
            num_layers,
            num_heads,
            ff_dim,
            dropout,
        );

        let p = vs.root();
        let value_head = nn::seq()
            .add(nn::linear(&p / "value_fc1", embed_dim, 64, Default::default()))
            .add_fn(|x| x.relu())
            .add(nn::linear(&p / "value_fc2", 64, 1, Default::default()));

        Self {
            transformer,
            value_head,
        }
    }

    /// Forward pass
    /// node_features: [batch, 19, input_dim]
    /// Returns: [batch, 1] value in [-1, 1]
    pub fn forward(&self, node_features: &Tensor, train: bool) -> Tensor {
        // Backbone: [batch, 19, embed_dim]
        let h = self.transformer.forward(node_features, train);

        // Mean pool over nodes: [batch, embed_dim]
        let pooled = h.mean_dim(1, false, Kind::Float);

        // Value head: [batch, 1]
        pooled.apply(&self.value_head).tanh()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_transformer_forward() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let transformer = GraphTransformer::new(&vs, 47, 128, 2, 4, 512, 0.1);

        let x = Tensor::randn([2, 19, 47], (Kind::Float, tch::Device::Cpu));
        let out = transformer.forward(&x, false);

        assert_eq!(out.size(), vec![2, 19, 128]);
    }

    #[test]
    fn test_graph_transformer_policy_net() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let policy = GraphTransformerPolicyNet::new(&vs, 47, 128, 2, 4, 0.1);

        let x = Tensor::randn([2, 19, 47], (Kind::Float, tch::Device::Cpu));
        let logits = policy.forward(&x, false);

        assert_eq!(logits.size(), vec![2, 19]);
    }

    #[test]
    fn test_positional_encoding() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let pos_enc = PositionalEncoding::new(&vs.root(), 128);

        let x = Tensor::randn([2, 19, 128], (Kind::Float, tch::Device::Cpu));
        let out = pos_enc.forward(&x);

        assert_eq!(out.size(), vec![2, 19, 128]);
    }

    #[test]
    fn test_graph_transformer_value_net() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let value = GraphTransformerValueNet::new(&vs, 47, 128, 2, 4, 0.1);

        let x = Tensor::randn([4, 19, 47], (Kind::Float, tch::Device::Cpu));
        let out = value.forward(&x, false);

        assert_eq!(out.size(), vec![4, 1]);
        // Output should be in [-1, 1] due to tanh
        let max_val: f64 = out.abs().max().double_value(&[]);
        assert!(max_val <= 1.0, "Value output should be in [-1, 1], got max abs {}", max_val);
    }

    #[test]
    fn test_multi_head_attention() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let attn = MultiHeadAttention::new(&vs.root(), 128, 4);

        let x = Tensor::randn([2, 19, 128], (Kind::Float, tch::Device::Cpu));
        let out = attn.forward(&x, false, 0.0);

        assert_eq!(out.size(), vec![2, 19, 128]);
    }
}
