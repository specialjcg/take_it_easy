//! Sheaf Neural Network for Take It Easy
//!
//! A sheaf on the hex board assigns learnable "restriction maps" per direction
//! that transform node features into a "stalk space" where consistency along
//! scoring lines can be measured. This is fundamentally different from attention:
//! instead of learning "which nodes to attend to", the sheaf learns
//! "how to project each position for each scoring direction."
//!
//! Key insight: each position belongs to exactly 3 scoring lines (one per direction).
//! The sheaf Laplacian measures disagreement: "how different is this position from
//! the mean of its line?" — directly encoding the scoring structure.
//!
//! Architecture per layer:
//!   1. For each direction d (0=column, 1=diag/, 2=diag\):
//!      a. Project all nodes to stalk space: S = R_d(X)
//!      b. Compute per-line mean in stalk space
//!      c. Disagreement = line_mean - node_stalk (sheaf Laplacian)
//!      d. Lift back to node space: msg_d = L_d(disagreement)
//!   2. Aggregate messages from 3 directions via gated fusion
//!   3. FFN + residual
//!
//! Novel properties vs existing architectures:
//!   - Direction-aware: learns different "views" per scoring axis
//!   - Topology-native: message passing follows the hypergraph structure
//!   - Interpretable: restriction maps show what the net extracts per direction
//!   - No attention mechanism: purely sheaf-theoretic message passing

use tch::{nn, Kind, Tensor};

const NODE_COUNT: i64 = 19;
const NUM_DIRECTIONS: usize = 3;
const LINES_PER_DIR: usize = 5;

/// Lines per direction: members of each line (padded to 5, -1 = pad)
const DIR_LINES: [[[i64; 5]; 5]; 3] = [
    // Direction 0: columns
    [
        [0, 1, 2, -1, -1],
        [3, 4, 5, 6, -1],
        [7, 8, 9, 10, 11],
        [12, 13, 14, 15, -1],
        [16, 17, 18, -1, -1],
    ],
    // Direction 1: diagonal /
    [
        [0, 3, 7, -1, -1],
        [1, 4, 8, 12, -1],
        [2, 5, 9, 13, 16],
        [6, 10, 14, 17, -1],
        [11, 15, 18, -1, -1],
    ],
    // Direction 2: diagonal \
    [
        [7, 12, 16, -1, -1],
        [3, 8, 13, 17, -1],
        [0, 4, 9, 14, 18],
        [1, 5, 10, 15, -1],
        [2, 6, 11, -1, -1],
    ],
];

const DIR_LINE_LENS: [[usize; 5]; 3] = [
    [3, 4, 5, 4, 3],
    [3, 4, 5, 4, 3],
    [3, 4, 5, 4, 3],
];

/// For each node (0..18), which line index (0..4) in each direction
const NODE_TO_DIR_LINE: [[usize; 3]; 19] = [
    [0, 0, 2], // node 0
    [0, 1, 3], // node 1
    [0, 2, 4], // node 2
    [1, 0, 1], // node 3
    [1, 1, 2], // node 4
    [1, 2, 3], // node 5
    [1, 3, 4], // node 6
    [2, 0, 0], // node 7
    [2, 1, 1], // node 8
    [2, 2, 2], // node 9
    [2, 3, 3], // node 10
    [2, 4, 4], // node 11
    [3, 1, 0], // node 12
    [3, 2, 1], // node 13
    [3, 3, 2], // node 14
    [3, 4, 3], // node 15
    [4, 2, 0], // node 16
    [4, 3, 1], // node 17
    [4, 4, 2], // node 18
];

/// Feed-forward block
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

/// One Sheaf Diffusion Layer
///
/// Message flow per direction d:
///   X --R_d--> S (stalk space)
///   S --line_mean--> disagreement
///   disagreement --L_d--> msg (node space)
/// Then: fuse 3 directions via gated residual + FFN
struct SheafLayer {
    // Per-direction restriction maps: node space -> stalk space
    restrict: Vec<nn::Linear>, // [3] embed_dim -> stalk_dim
    // Per-direction lift maps: stalk space -> node space
    lift: Vec<nn::Linear>, // [3] stalk_dim -> embed_dim

    // Gated fusion
    diffusion_ln: nn::LayerNorm,
    gate_proj: nn::Linear, // 2*embed_dim -> embed_dim

    // FFN
    ffn_ln: nn::LayerNorm,
    ffn: FFN,
}

impl SheafLayer {
    fn new(path: &nn::Path, embed_dim: i64, stalk_dim: i64, ff_dim: i64) -> Self {
        let mut restrict = Vec::with_capacity(NUM_DIRECTIONS);
        let mut lift = Vec::with_capacity(NUM_DIRECTIONS);
        for d in 0..NUM_DIRECTIONS {
            restrict.push(nn::linear(
                path / format!("restrict_{}", d),
                embed_dim,
                stalk_dim,
                Default::default(),
            ));
            lift.push(nn::linear(
                path / format!("lift_{}", d),
                stalk_dim,
                embed_dim,
                Default::default(),
            ));
        }

        Self {
            restrict,
            lift,
            diffusion_ln: nn::layer_norm(
                path / "diff_ln",
                vec![embed_dim],
                nn::LayerNormConfig {
                    eps: 1e-5,
                    ..Default::default()
                },
            ),
            gate_proj: nn::linear(
                path / "gate",
                embed_dim * 2,
                embed_dim,
                Default::default(),
            ),
            ffn_ln: nn::layer_norm(
                path / "ffn_ln",
                vec![embed_dim],
                nn::LayerNormConfig {
                    eps: 1e-5,
                    ..Default::default()
                },
            ),
            ffn: FFN::new(&(path / "ffn"), embed_dim, ff_dim),
        }
    }

    /// Compute sheaf diffusion messages for one direction
    /// Returns: [B, 19, embed_dim] messages
    fn sheaf_diffusion_dir(&self, x: &Tensor, dir: usize) -> Tensor {
        let device = x.device();

        // Project to stalk space: [B, 19, stalk_dim]
        let s = x.apply(&self.restrict[dir]);

        // Compute line means in stalk space
        let mut line_means: Vec<Tensor> = Vec::with_capacity(LINES_PER_DIR);
        for line_idx in 0..LINES_PER_DIR {
            let len = DIR_LINE_LENS[dir][line_idx];
            let members: Vec<i64> = DIR_LINES[dir][line_idx][..len].to_vec();
            let idx = Tensor::from_slice(&members).to_device(device);
            let line_stalks = s.index_select(1, &idx); // [B, len, stalk_dim]
            line_means.push(line_stalks.mean_dim(1, false, Kind::Float)); // [B, stalk_dim]
        }

        // For each node: disagreement = line_mean - node_stalk
        let mut disagree_parts: Vec<Tensor> = Vec::with_capacity(19);
        for node in 0..19 {
            let line_idx = NODE_TO_DIR_LINE[node][dir];
            let node_stalk = s.select(1, node as i64); // [B, stalk_dim]
            let disagree = &line_means[line_idx] - &node_stalk; // [B, stalk_dim]
            disagree_parts.push(disagree.unsqueeze(1)); // [B, 1, stalk_dim]
        }

        // Stack: [B, 19, stalk_dim]
        let all_disagree = Tensor::cat(&disagree_parts, 1);

        // Lift back to node space: [B, 19, embed_dim]
        all_disagree.apply(&self.lift[dir])
    }

    /// Forward: nodes [B, 19, D] -> nodes [B, 19, D]
    fn forward(&self, x: &Tensor, train: bool, dropout: f64) -> Tensor {
        // 1. Sheaf diffusion from all 3 directions
        let msg0 = self.sheaf_diffusion_dir(x, 0);
        let msg1 = self.sheaf_diffusion_dir(x, 1);
        let msg2 = self.sheaf_diffusion_dir(x, 2);
        let msg = msg0 + msg1 + msg2;

        // 2. Gated residual
        let normed = x.apply(&self.diffusion_ln);
        let gate_input = Tensor::cat(&[&normed, &msg], -1);
        let gate = gate_input.apply(&self.gate_proj).sigmoid();
        let x = x + &gate * &msg;
        let x = if train && dropout > 0.0 {
            x.dropout(dropout, train)
        } else {
            x
        };

        // 3. FFN with residual
        let normed = x.apply(&self.ffn_ln);
        let ffn_out = self.ffn.forward(&normed, train, dropout);
        let ffn_out = if train && dropout > 0.0 {
            ffn_out.dropout(dropout, train)
        } else {
            ffn_out
        };
        x + ffn_out
    }
}

/// Multi-head self-attention block
struct MultiHeadAttention {
    qkv: nn::Linear,
    out_proj: nn::Linear,
    num_heads: i64,
    head_dim: i64,
}

impl MultiHeadAttention {
    fn new(path: &nn::Path, embed_dim: i64, num_heads: i64) -> Self {
        let head_dim = embed_dim / num_heads;
        Self {
            qkv: nn::linear(path / "qkv", embed_dim, embed_dim * 3, Default::default()),
            out_proj: nn::linear(path / "out", embed_dim, embed_dim, Default::default()),
            num_heads,
            head_dim,
        }
    }

    fn forward(&self, x: &Tensor, train: bool, dropout: f64) -> Tensor {
        let (b, n, _d) = x.size3().unwrap();
        let qkv = x.apply(&self.qkv); // [B, N, 3*D]
        let qkv = qkv.reshape([b, n, 3, self.num_heads, self.head_dim]);
        let qkv = qkv.permute([2, 0, 3, 1, 4]); // [3, B, H, N, Hd]
        let q = qkv.select(0, 0);
        let k = qkv.select(0, 1);
        let v = qkv.select(0, 2);

        let scale = (self.head_dim as f64).sqrt();
        let attn = q.matmul(&k.transpose(-2, -1)) / scale;
        let attn = attn.softmax(-1, Kind::Float);
        let attn = if train && dropout > 0.0 {
            attn.dropout(dropout, train)
        } else {
            attn
        };

        let out = attn.matmul(&v); // [B, H, N, Hd]
        let out = out.permute([0, 2, 1, 3]).reshape([b, n, -1]); // [B, N, D]
        out.apply(&self.out_proj)
    }
}

/// Sheaf + Attention hybrid layer
/// Sheaf diffusion captures line structure, attention captures global interactions
struct SheafAttentionLayer {
    // Sheaf diffusion (same as SheafLayer)
    restrict: Vec<nn::Linear>,
    lift: Vec<nn::Linear>,
    diffusion_ln: nn::LayerNorm,
    gate_proj: nn::Linear,

    // Self-attention
    attn_ln: nn::LayerNorm,
    attn: MultiHeadAttention,

    // FFN
    ffn_ln: nn::LayerNorm,
    ffn: FFN,
}

impl SheafAttentionLayer {
    fn new(path: &nn::Path, embed_dim: i64, stalk_dim: i64, ff_dim: i64, num_heads: i64) -> Self {
        let mut restrict = Vec::with_capacity(NUM_DIRECTIONS);
        let mut lift = Vec::with_capacity(NUM_DIRECTIONS);
        for d in 0..NUM_DIRECTIONS {
            restrict.push(nn::linear(
                path / format!("restrict_{}", d),
                embed_dim,
                stalk_dim,
                Default::default(),
            ));
            lift.push(nn::linear(
                path / format!("lift_{}", d),
                stalk_dim,
                embed_dim,
                Default::default(),
            ));
        }

        Self {
            restrict,
            lift,
            diffusion_ln: nn::layer_norm(
                path / "diff_ln",
                vec![embed_dim],
                nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
            ),
            gate_proj: nn::linear(path / "gate", embed_dim * 2, embed_dim, Default::default()),
            attn_ln: nn::layer_norm(
                path / "attn_ln",
                vec![embed_dim],
                nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
            ),
            attn: MultiHeadAttention::new(&(path / "attn"), embed_dim, num_heads),
            ffn_ln: nn::layer_norm(
                path / "ffn_ln",
                vec![embed_dim],
                nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
            ),
            ffn: FFN::new(&(path / "ffn"), embed_dim, ff_dim),
        }
    }

    fn sheaf_diffusion_dir(&self, x: &Tensor, dir: usize) -> Tensor {
        let device = x.device();
        let s = x.apply(&self.restrict[dir]);

        let mut line_means: Vec<Tensor> = Vec::with_capacity(LINES_PER_DIR);
        for line_idx in 0..LINES_PER_DIR {
            let len = DIR_LINE_LENS[dir][line_idx];
            let members: Vec<i64> = DIR_LINES[dir][line_idx][..len].to_vec();
            let idx = Tensor::from_slice(&members).to_device(device);
            let line_stalks = s.index_select(1, &idx);
            line_means.push(line_stalks.mean_dim(1, false, Kind::Float));
        }

        let mut disagree_parts: Vec<Tensor> = Vec::with_capacity(19);
        for node in 0..19 {
            let line_idx = NODE_TO_DIR_LINE[node][dir];
            let node_stalk = s.select(1, node as i64);
            let disagree = &line_means[line_idx] - &node_stalk;
            disagree_parts.push(disagree.unsqueeze(1));
        }

        let all_disagree = Tensor::cat(&disagree_parts, 1);
        all_disagree.apply(&self.lift[dir])
    }

    fn forward(&self, x: &Tensor, train: bool, dropout: f64) -> Tensor {
        // 1. Sheaf diffusion + gated residual
        let msg0 = self.sheaf_diffusion_dir(x, 0);
        let msg1 = self.sheaf_diffusion_dir(x, 1);
        let msg2 = self.sheaf_diffusion_dir(x, 2);
        let msg = msg0 + msg1 + msg2;

        let normed = x.apply(&self.diffusion_ln);
        let gate_input = Tensor::cat(&[&normed, &msg], -1);
        let gate = gate_input.apply(&self.gate_proj).sigmoid();
        let x = x + &gate * &msg;
        let x = if train && dropout > 0.0 { x.dropout(dropout, train) } else { x };

        // 2. Self-attention + residual
        let normed = x.apply(&self.attn_ln);
        let attn_out = self.attn.forward(&normed, train, dropout);
        let attn_out = if train && dropout > 0.0 { attn_out.dropout(dropout, train) } else { attn_out };
        let x = &x + attn_out;

        // 3. FFN + residual
        let normed = x.apply(&self.ffn_ln);
        let ffn_out = self.ffn.forward(&normed, train, dropout);
        let ffn_out = if train && dropout > 0.0 { ffn_out.dropout(dropout, train) } else { ffn_out };
        x + ffn_out
    }
}

/// Sheaf + Attention Hybrid Policy Net
pub struct SheafAttentionPolicyNet {
    input_proj: nn::Linear,
    pos_embed: Tensor,
    layers: Vec<SheafAttentionLayer>,
    final_ln: nn::LayerNorm,
    policy_head: nn::Linear,
    dropout: f64,
}

impl SheafAttentionPolicyNet {
    pub fn new(
        vs: &nn::VarStore,
        input_dim: i64,
        embed_dim: i64,
        stalk_dim: i64,
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
            layers.push(SheafAttentionLayer::new(
                &(&p / format!("sheaf_attn_{}", i)),
                embed_dim, stalk_dim, ff_dim, num_heads,
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

/// Sheaf Neural Network Policy Net
pub struct SheafPolicyNet {
    input_proj: nn::Linear,
    pos_embed: Tensor,
    layers: Vec<SheafLayer>,
    final_ln: nn::LayerNorm,
    policy_head: nn::Linear,
    dropout: f64,
}

impl SheafPolicyNet {
    pub fn new(
        vs: &nn::VarStore,
        input_dim: i64,
        embed_dim: i64,
        stalk_dim: i64,
        num_layers: usize,
        dropout: f64,
    ) -> Self {
        let p = vs.root();
        let ff_dim = embed_dim * 4;

        let input_proj = nn::linear(&p / "input_proj", input_dim, embed_dim, Default::default());
        let pos_embed = (&p / "pos_embed").var(
            "weight",
            &[NODE_COUNT, embed_dim],
            nn::Init::Randn {
                mean: 0.0,
                stdev: 0.02,
            },
        );

        let mut layers = Vec::with_capacity(num_layers);
        for i in 0..num_layers {
            layers.push(SheafLayer::new(
                &(&p / format!("sheaf_{}", i)),
                embed_dim,
                stalk_dim,
                ff_dim,
            ));
        }

        let final_ln = nn::layer_norm(
            &p / "final_ln",
            vec![embed_dim],
            nn::LayerNormConfig {
                eps: 1e-5,
                ..Default::default()
            },
        );
        let policy_head = nn::linear(&p / "policy_head", embed_dim, 1, Default::default());

        Self {
            input_proj,
            pos_embed,
            layers,
            final_ln,
            policy_head,
            dropout,
        }
    }

    /// node_features: [B, 19, input_dim] -> [B, 19] logits
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
    fn test_sheaf_policy_forward() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let net = SheafPolicyNet::new(&vs, 47, 128, 64, 2, 0.1);

        let x = Tensor::randn([4, 19, 47], (Kind::Float, tch::Device::Cpu));
        let logits = net.forward(&x, false);
        assert_eq!(logits.size(), vec![4, 19]);
    }

    #[test]
    fn test_sheaf_layer_forward() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let layer = SheafLayer::new(&vs.root(), 64, 32, 256);

        let nodes = Tensor::randn([2, 19, 64], (Kind::Float, tch::Device::Cpu));
        let out = layer.forward(&nodes, false, 0.0);
        assert_eq!(out.size(), vec![2, 19, 64]);
    }

    #[test]
    fn test_sheaf_diffusion_single_dir() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let layer = SheafLayer::new(&vs.root(), 64, 32, 256);

        let nodes = Tensor::randn([2, 19, 64], (Kind::Float, tch::Device::Cpu));
        let msg = layer.sheaf_diffusion_dir(&nodes, 0);
        assert_eq!(msg.size(), vec![2, 19, 64]);
    }

    #[test]
    fn test_node_to_dir_line_consistency() {
        // Verify every node appears in exactly the line it claims
        for node in 0..19 {
            for dir in 0..3 {
                let line_idx = NODE_TO_DIR_LINE[node][dir];
                let len = DIR_LINE_LENS[dir][line_idx];
                let members = &DIR_LINES[dir][line_idx][..len];
                assert!(
                    members.contains(&(node as i64)),
                    "Node {} not found in dir {} line {} (members: {:?})",
                    node,
                    dir,
                    line_idx,
                    members
                );
            }
        }
    }

    #[test]
    fn test_every_node_in_exactly_one_line_per_dir() {
        for dir in 0..3 {
            let mut node_count = vec![0usize; 19];
            for line_idx in 0..5 {
                let len = DIR_LINE_LENS[dir][line_idx];
                for &node in &DIR_LINES[dir][line_idx][..len] {
                    node_count[node as usize] += 1;
                }
            }
            for (node, &count) in node_count.iter().enumerate() {
                assert_eq!(
                    count, 1,
                    "Node {} appears {} times in direction {}",
                    node, count, dir
                );
            }
        }
    }

    #[test]
    fn test_sheaf_attention_forward() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let net = SheafAttentionPolicyNet::new(&vs, 47, 128, 64, 2, 4, 0.1);

        let x = Tensor::randn([4, 19, 47], (Kind::Float, tch::Device::Cpu));
        let logits = net.forward(&x, false);
        assert_eq!(logits.size(), vec![4, 19]);
    }

    #[test]
    fn test_sheaf_attention_param_count() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let _net = SheafAttentionPolicyNet::new(&vs, 47, 128, 64, 3, 4, 0.1);
        let count: i64 = vs.variables().values().map(|t| t.numel() as i64).sum();
        println!("SheafAttention param count (dim=128, stalk=64, 3 layers, 4 heads): {}", count);
        assert!(count > 0);
    }

    #[test]
    fn test_param_count() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let _net = SheafPolicyNet::new(&vs, 47, 128, 64, 2, 0.1);
        let count: i64 = vs.variables().values().map(|t| t.numel() as i64).sum();
        println!("Sheaf param count (dim=128, stalk=64, 2 layers): {}", count);
        assert!(count > 0);
    }
}
