//! Edge-Aware Graph Transformer for Take It Easy
//!
//! Enhances the standard Graph Transformer by injecting structural bias into
//! attention scores. For each pair of positions (i, j), the model knows:
//!   - Whether they share a scoring line in direction 0, 1, or 2
//!   - The length of the shared line (3, 4, or 5 positions)
//!   - The hop distance on the hex adjacency graph
//!
//! These edge features are projected per-head into attention bias, allowing
//! different heads to specialize on different structural relationships
//! (e.g., head 0 focuses on column mates, head 1 on diagonal mates).
//!
//! Key insight: the vanilla GT must discover line-sharing structure purely
//! from data via positional embeddings. Edge-Aware GT provides this as a
//! strong prior, freeing capacity for learning finer-grained patterns.

use tch::{nn, Kind, Tensor};

const NODE_COUNT: i64 = 19;
const NUM_EDGE_FEATURES: i64 = 7; // 3 share_dir + 3 line_len + 1 distance

/// Line definitions: (positions, direction)
/// Duplicated here to avoid cross-module dependency
const LINE_DEFS: [(&[usize], usize); 15] = [
    (&[0, 1, 2], 0),
    (&[3, 4, 5, 6], 0),
    (&[7, 8, 9, 10, 11], 0),
    (&[12, 13, 14, 15], 0),
    (&[16, 17, 18], 0),
    (&[0, 3, 7], 1),
    (&[1, 4, 8, 12], 1),
    (&[2, 5, 9, 13, 16], 1),
    (&[6, 10, 14, 17], 1),
    (&[11, 15, 18], 1),
    (&[7, 12, 16], 2),
    (&[3, 8, 13, 17], 2),
    (&[0, 4, 9, 14, 18], 2),
    (&[1, 5, 10, 15], 2),
    (&[2, 6, 11], 2),
];

/// Hex adjacency edges (undirected)
const HEX_EDGES: &[(usize, usize)] = &[
    (0, 1), (1, 2),
    (0, 3), (1, 4), (2, 5), (2, 6),
    (3, 4), (4, 5), (5, 6),
    (3, 7), (4, 8), (5, 9), (6, 10), (6, 11),
    (7, 8), (8, 9), (9, 10), (10, 11),
    (7, 12), (8, 13), (9, 14), (10, 15),
    (12, 13), (13, 14), (14, 15),
    (12, 16), (13, 17), (14, 18),
    (16, 17), (17, 18),
];

/// Compute BFS shortest path distances between all pairs of nodes
fn compute_distances() -> [[f32; 19]; 19] {
    let mut dist = [[f32::MAX; 19]; 19];
    for i in 0..19 {
        dist[i][i] = 0.0;
    }
    for &(a, b) in HEX_EDGES {
        dist[a][b] = 1.0;
        dist[b][a] = 1.0;
    }
    // Floyd-Warshall
    for k in 0..19 {
        for i in 0..19 {
            for j in 0..19 {
                if dist[i][k] + dist[k][j] < dist[i][j] {
                    dist[i][j] = dist[i][k] + dist[k][j];
                }
            }
        }
    }
    dist
}

/// Pre-compute edge features matrix [19, 19, 7]
/// Features: [share_dir0, share_dir1, share_dir2, line_len_dir0, line_len_dir1, line_len_dir2, distance]
fn compute_edge_features() -> Vec<f32> {
    let distances = compute_distances();
    let max_dist = 6.0; // max BFS distance on hex board

    // For each pair, check which lines they share
    let mut features = vec![0.0f32; 19 * 19 * NUM_EDGE_FEATURES as usize];

    for i in 0..19 {
        for j in 0..19 {
            let base = (i * 19 + j) * NUM_EDGE_FEATURES as usize;

            // Check each line
            for &(positions, dir) in &LINE_DEFS {
                let i_in = positions.contains(&i);
                let j_in = positions.contains(&j);
                if i_in && j_in {
                    // share_dir[d] = 1.0
                    features[base + dir] = 1.0;
                    // line_len_dir[d] = len / 5.0 (normalized)
                    features[base + 3 + dir] = positions.len() as f32 / 5.0;
                }
            }

            // Normalized distance
            features[base + 6] = distances[i][j] / max_dist;
        }
    }

    features
}

/// Edge-Aware Multi-Head Attention
///
/// Standard MHA + learned structural bias from edge features.
/// edge_bias[h, i, j] = edge_features[i, j, :] @ edge_proj[:, h]
struct EdgeAwareAttention {
    q_proj: nn::Linear,
    k_proj: nn::Linear,
    v_proj: nn::Linear,
    out_proj: nn::Linear,
    edge_proj: nn::Linear, // [NUM_EDGE_FEATURES, num_heads]
    num_heads: i64,
    head_dim: i64,
    scale: f64,
    edge_features: Tensor, // [19, 19, 7] pre-computed, fixed
}

impl EdgeAwareAttention {
    fn new(path: &nn::Path, embed_dim: i64, num_heads: i64) -> Self {
        let head_dim = embed_dim / num_heads;
        let scale = (head_dim as f64).sqrt();

        let edge_features_data = compute_edge_features();
        let edge_features = Tensor::from_slice(&edge_features_data)
            .view([NODE_COUNT, NODE_COUNT, NUM_EDGE_FEATURES]);

        Self {
            q_proj: nn::linear(path / "q_proj", embed_dim, embed_dim, Default::default()),
            k_proj: nn::linear(path / "k_proj", embed_dim, embed_dim, Default::default()),
            v_proj: nn::linear(path / "v_proj", embed_dim, embed_dim, Default::default()),
            out_proj: nn::linear(path / "out_proj", embed_dim, embed_dim, Default::default()),
            edge_proj: nn::linear(
                path / "edge_proj",
                NUM_EDGE_FEATURES,
                num_heads,
                nn::LinearConfig { bias: false, ..Default::default() },
            ),
            num_heads,
            head_dim,
            scale,
            edge_features,
        }
    }

    /// Forward with structural bias
    /// x: [B, 19, D] → [B, 19, D]
    fn forward(&self, x: &Tensor, train: bool, dropout: f64) -> Tensor {
        let (b, n, _) = x.size3().unwrap();

        let q = x.apply(&self.q_proj)
            .view([b, n, self.num_heads, self.head_dim])
            .permute([0, 2, 1, 3]); // [B, H, N, Hd]
        let k = x.apply(&self.k_proj)
            .view([b, n, self.num_heads, self.head_dim])
            .permute([0, 2, 1, 3]);
        let v = x.apply(&self.v_proj)
            .view([b, n, self.num_heads, self.head_dim])
            .permute([0, 2, 1, 3]);

        // Standard attention scores
        let attn_scores = q.matmul(&k.transpose(-2, -1)) / self.scale; // [B, H, N, N]

        // Structural bias: edge_features[19, 19, 7] @ edge_proj[7, H] → [19, 19, H]
        let ef = self.edge_features.to_device(x.device());
        let bias = ef.apply(&self.edge_proj); // [19, 19, H]
        let bias = bias.permute([2, 0, 1]).unsqueeze(0); // [1, H, 19, 19]

        let attn_scores = attn_scores + bias;

        let attn_weights = attn_scores.softmax(-1, Kind::Float);
        let attn_weights = if train && dropout > 0.0 {
            attn_weights.dropout(dropout, train)
        } else {
            attn_weights
        };

        let out = attn_weights.matmul(&v); // [B, H, N, Hd]
        let out = out.permute([0, 2, 1, 3]).contiguous()
            .view([b, n, self.num_heads * self.head_dim]);

        out.apply(&self.out_proj)
    }
}

/// FFN block
struct FFN {
    fc1: nn::Linear,
    fc2: nn::Linear,
}

impl FFN {
    fn new(path: &nn::Path, dim: i64, ff_dim: i64) -> Self {
        Self {
            fc1: nn::linear(path / "fc1", dim, ff_dim, Default::default()),
            fc2: nn::linear(path / "fc2", ff_dim, dim, Default::default()),
        }
    }

    fn forward(&self, x: &Tensor, train: bool, dropout: f64) -> Tensor {
        let h = x.apply(&self.fc1).gelu("none");
        let h = if train && dropout > 0.0 {
            h.dropout(dropout, train)
        } else {
            h
        };
        h.apply(&self.fc2)
    }
}

/// Edge-Aware Transformer Layer
struct EdgeAwareTransformerLayer {
    attn: EdgeAwareAttention,
    ffn: FFN,
    ln1: nn::LayerNorm,
    ln2: nn::LayerNorm,
}

impl EdgeAwareTransformerLayer {
    fn new(path: &nn::Path, embed_dim: i64, num_heads: i64, ff_dim: i64) -> Self {
        Self {
            attn: EdgeAwareAttention::new(&(path / "attn"), embed_dim, num_heads),
            ffn: FFN::new(&(path / "ffn"), embed_dim, ff_dim),
            ln1: nn::layer_norm(
                path / "ln1",
                vec![embed_dim],
                nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
            ),
            ln2: nn::layer_norm(
                path / "ln2",
                vec![embed_dim],
                nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
            ),
        }
    }

    fn forward(&self, x: &Tensor, train: bool, dropout: f64) -> Tensor {
        // Pre-LN attention + residual
        let normed = x.apply(&self.ln1);
        let attn_out = self.attn.forward(&normed, train, dropout);
        let attn_out = if train && dropout > 0.0 {
            attn_out.dropout(dropout, train)
        } else {
            attn_out
        };
        let x = x + attn_out;

        // Pre-LN FFN + residual
        let normed = x.apply(&self.ln2);
        let ffn_out = self.ffn.forward(&normed, train, dropout);
        let ffn_out = if train && dropout > 0.0 {
            ffn_out.dropout(dropout, train)
        } else {
            ffn_out
        };
        x + ffn_out
    }
}

/// Edge-Aware Graph Transformer Policy Network
pub struct EdgeAwareGTPolicyNet {
    input_proj: nn::Linear,
    pos_embed: Tensor,
    layers: Vec<EdgeAwareTransformerLayer>,
    final_ln: nn::LayerNorm,
    policy_head: nn::Linear,
    dropout: f64,
}

impl EdgeAwareGTPolicyNet {
    pub fn new(
        vs: &nn::VarStore,
        input_dim: i64,
        embed_dim: i64,
        num_layers: usize,
        num_heads: i64,
        dropout: f64,
    ) -> Self {
        let p = vs.root();
        let ff_dim = embed_dim * 4;

        let input_proj = nn::linear(&p / "input_proj", input_dim, embed_dim, Default::default());
        let pos_embed = (&p / "pos_embed").var(
            "weight",
            &[NODE_COUNT, embed_dim],
            nn::Init::Randn { mean: 0.0, stdev: 0.02 },
        );

        let mut layers = Vec::with_capacity(num_layers);
        for i in 0..num_layers {
            layers.push(EdgeAwareTransformerLayer::new(
                &(&p / format!("layer_{}", i)),
                embed_dim, num_heads, ff_dim,
            ));
        }

        let final_ln = nn::layer_norm(
            &p / "final_ln",
            vec![embed_dim],
            nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
        );
        let policy_head = nn::linear(&p / "policy_head", embed_dim, 1, Default::default());

        Self { input_proj, pos_embed, layers, final_ln, policy_head, dropout }
    }

    /// node_features: [B, 19, input_dim] → [B, 19] logits
    pub fn forward(&self, node_features: &Tensor, train: bool) -> Tensor {
        let mut h = node_features.apply(&self.input_proj) + self.pos_embed.unsqueeze(0);
        if train && self.dropout > 0.0 {
            h = h.dropout(self.dropout, train);
        }

        for layer in &self.layers {
            h = layer.forward(&h, train, self.dropout);
        }

        h = h.apply(&self.final_ln);
        h.apply(&self.policy_head).squeeze_dim(-1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_features_shape() {
        let ef = compute_edge_features();
        assert_eq!(ef.len(), 19 * 19 * 7);
    }

    #[test]
    fn test_edge_features_self_loops() {
        let ef = compute_edge_features();
        // Self-loops: all 3 directions should be shared (position shares all its own lines)
        // Distance to self = 0
        for i in 0..19 {
            let base = (i * 19 + i) * 7;
            // Distance should be 0
            assert_eq!(ef[base + 6], 0.0, "Self-distance should be 0 for node {}", i);
        }
    }

    #[test]
    fn test_edge_features_center_shares() {
        let ef = compute_edge_features();
        // Position 9 (center) shares Dir 0 line with 7, 8, 10, 11
        // Check 9-8 shares Dir 0
        let base = (9 * 19 + 8) * 7;
        assert_eq!(ef[base], 1.0, "9 and 8 should share Dir 0 line");
        // And line length = 5/5 = 1.0
        assert_eq!(ef[base + 3], 1.0, "9-8 Dir 0 line length should be 5/5");
    }

    #[test]
    fn test_distances_symmetric() {
        let dist = compute_distances();
        for i in 0..19 {
            for j in 0..19 {
                assert_eq!(dist[i][j], dist[j][i], "Distance should be symmetric");
            }
        }
    }

    #[test]
    fn test_edge_aware_gt_forward() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let net = EdgeAwareGTPolicyNet::new(&vs, 47, 64, 2, 4, 0.1);
        let x = Tensor::randn([4, 19, 47], (Kind::Float, tch::Device::Cpu));
        let out = net.forward(&x, false);
        assert_eq!(out.size(), vec![4, 19]);
    }

    #[test]
    fn test_edge_aware_gt_train_mode() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let net = EdgeAwareGTPolicyNet::new(&vs, 47, 64, 2, 4, 0.1);
        let x = Tensor::randn([4, 19, 47], (Kind::Float, tch::Device::Cpu));
        let out = net.forward(&x, true);
        assert_eq!(out.size(), vec![4, 19]);
    }

    #[test]
    fn test_edge_aware_gt_param_count() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let _net = EdgeAwareGTPolicyNet::new(&vs, 47, 128, 2, 4, 0.1);
        let count: i64 = vs.variables().values().map(|t| t.numel() as i64).sum();
        println!("Edge-Aware GT params: {}", count);
        // Similar to vanilla GT (~137K for dim=128/2layers) but with edge_proj per layer
        // dim=128, 2 layers → ~400K (edge_proj adds [7, num_heads] per layer)
        assert!(count > 100_000, "Expected significant params, got {}", count);
        println!("  (vanilla GT ~137K for same config)");
    }
}
