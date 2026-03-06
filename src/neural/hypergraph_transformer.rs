//! Hypergraph Transformer for Take It Easy
//!
//! Novel architecture that treats BOTH positions (19 nodes) AND scoring lines
//! (15 hyperedges) as first-class entities. The game's structure is a hypergraph:
//! each position belongs to exactly 3 scoring lines, each line contains 3-5 positions.
//!
//! Architecture per layer:
//!   1. Node Self-Attention: standard MHA between 19 positions
//!   2. Node -> Line Aggregation: pool member nodes into line embeddings
//!   3. Line Self-Attention: MHA between 15 scoring lines
//!   4. Line -> Node Cross-Attention: scatter line info back to positions
//!   5. Fusion Gate: learn to blend node-level and line-level information
//!
//! This lets the model reason about "which LINE to complete" (strategic)
//! rather than just "which POSITION to fill" (tactical).

use tch::{nn, Kind, Tensor};

const NODE_COUNT: i64 = 19;
const LINE_COUNT: i64 = 15;

/// Which positions belong to each line (padded to max length 5, -1 = padding)
const LINE_MEMBERS: [[i64; 5]; 15] = [
    [0, 1, 2, -1, -1],       // line 0:  col0 (dir0)
    [3, 4, 5, 6, -1],        // line 1:  col1 (dir0)
    [7, 8, 9, 10, 11],       // line 2:  col2 (dir0)
    [12, 13, 14, 15, -1],    // line 3:  col3 (dir0)
    [16, 17, 18, -1, -1],    // line 4:  col4 (dir0)
    [0, 3, 7, -1, -1],       // line 5:  diag1-a (dir1)
    [1, 4, 8, 12, -1],       // line 6:  diag1-b (dir1)
    [2, 5, 9, 13, 16],       // line 7:  diag1-c (dir1)
    [6, 10, 14, 17, -1],     // line 8:  diag1-d (dir1)
    [11, 15, 18, -1, -1],    // line 9:  diag1-e (dir1)
    [7, 12, 16, -1, -1],     // line 10: diag2-a (dir2)
    [3, 8, 13, 17, -1],      // line 11: diag2-b (dir2)
    [0, 4, 9, 14, 18],       // line 12: diag2-c (dir2)
    [1, 5, 10, 15, -1],      // line 13: diag2-d (dir2)
    [2, 6, 11, -1, -1],      // line 14: diag2-e (dir2)
];

const LINE_LENGTHS: [i64; 15] = [3, 4, 5, 4, 3, 3, 4, 5, 4, 3, 3, 4, 5, 4, 3];

/// Multi-head attention (reusable for both node and line levels)
struct MHA {
    q: nn::Linear,
    k: nn::Linear,
    v: nn::Linear,
    out: nn::Linear,
    num_heads: i64,
    head_dim: i64,
    scale: f64,
}

impl MHA {
    fn new(path: &nn::Path, dim: i64, num_heads: i64) -> Self {
        let head_dim = dim / num_heads;
        Self {
            q: nn::linear(path / "q", dim, dim, Default::default()),
            k: nn::linear(path / "k", dim, dim, Default::default()),
            v: nn::linear(path / "v", dim, dim, Default::default()),
            out: nn::linear(path / "out", dim, dim, Default::default()),
            num_heads,
            head_dim,
            scale: (head_dim as f64).sqrt(),
        }
    }

    /// Self-attention: x: [B, S, D] -> [B, S, D]
    fn forward(&self, x: &Tensor, train: bool, dropout: f64) -> Tensor {
        let (b, s, _) = x.size3().unwrap();
        let reshape = |t: Tensor| {
            t.view([b, s, self.num_heads, self.head_dim])
                .permute([0, 2, 1, 3])
        };

        let q = reshape(x.apply(&self.q));
        let k = reshape(x.apply(&self.k));
        let v = reshape(x.apply(&self.v));

        let attn = q.matmul(&k.transpose(-2, -1)) / self.scale;
        let attn = attn.softmax(-1, Kind::Float);
        let attn = if train && dropout > 0.0 {
            attn.dropout(dropout, train)
        } else {
            attn
        };

        let out = attn
            .matmul(&v)
            .permute([0, 2, 1, 3])
            .contiguous()
            .view([b, s, self.num_heads * self.head_dim]);
        out.apply(&self.out)
    }

    /// Cross-attention: q from x, k/v from ctx
    /// x: [B, Sq, D], ctx: [B, Sc, D] -> [B, Sq, D]
    fn cross_forward(&self, x: &Tensor, ctx: &Tensor, train: bool, dropout: f64) -> Tensor {
        let (b, sq, _) = x.size3().unwrap();
        let sc = ctx.size()[1];

        let q = x
            .apply(&self.q)
            .view([b, sq, self.num_heads, self.head_dim])
            .permute([0, 2, 1, 3]);
        let k = ctx
            .apply(&self.k)
            .view([b, sc, self.num_heads, self.head_dim])
            .permute([0, 2, 1, 3]);
        let v = ctx
            .apply(&self.v)
            .view([b, sc, self.num_heads, self.head_dim])
            .permute([0, 2, 1, 3]);

        let attn = q.matmul(&k.transpose(-2, -1)) / self.scale;
        let attn = attn.softmax(-1, Kind::Float);
        let attn = if train && dropout > 0.0 {
            attn.dropout(dropout, train)
        } else {
            attn
        };

        let out = attn
            .matmul(&v)
            .permute([0, 2, 1, 3])
            .contiguous()
            .view([b, sq, self.num_heads * self.head_dim]);
        out.apply(&self.out)
    }
}

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

/// One Hypergraph Transformer layer
///
/// Message flow:
///   nodes --self-attn--> nodes'
///   nodes' --aggregate--> lines
///   lines  --self-attn--> lines'
///   lines' --cross-attn--> nodes''
///   nodes'' = gate(nodes', cross_out) + FFN
struct HyperLayer {
    // Node self-attention
    node_attn: MHA,
    node_ln1: nn::LayerNorm,

    // Node -> Line aggregation
    line_proj: nn::Linear,
    line_type_embed: Tensor,  // [3, D] for 3 directions
    line_len_embed: Tensor,   // [3, D] for lengths 3,4,5

    // Line self-attention
    line_attn: MHA,
    line_ln: nn::LayerNorm,

    // Line -> Node cross-attention
    cross_attn: MHA,
    cross_ln: nn::LayerNorm,

    // Fusion gate
    gate_proj: nn::Linear,

    // FFN
    ffn: FFN,
    ffn_ln: nn::LayerNorm,
}

impl HyperLayer {
    fn new(path: &nn::Path, embed_dim: i64, num_heads: i64, ff_dim: i64) -> Self {
        Self {
            node_attn: MHA::new(&(path / "node_attn"), embed_dim, num_heads),
            node_ln1: nn::layer_norm(
                path / "node_ln1",
                vec![embed_dim],
                nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
            ),

            line_proj: nn::linear(path / "line_proj", embed_dim, embed_dim, Default::default()),
            line_type_embed: path.var(
                "line_type_embed",
                &[3, embed_dim],
                nn::Init::Randn { mean: 0.0, stdev: 0.02 },
            ),
            line_len_embed: path.var(
                "line_len_embed",
                &[3, embed_dim],
                nn::Init::Randn { mean: 0.0, stdev: 0.02 },
            ),

            line_attn: MHA::new(&(path / "line_attn"), embed_dim, num_heads),
            line_ln: nn::layer_norm(
                path / "line_ln",
                vec![embed_dim],
                nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
            ),

            cross_attn: MHA::new(&(path / "cross_attn"), embed_dim, num_heads),
            cross_ln: nn::layer_norm(
                path / "cross_ln",
                vec![embed_dim],
                nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
            ),

            gate_proj: nn::linear(path / "gate", embed_dim * 2, embed_dim, Default::default()),

            ffn: FFN::new(&(path / "ffn"), embed_dim, ff_dim),
            ffn_ln: nn::layer_norm(
                path / "ffn_ln",
                vec![embed_dim],
                nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
            ),
        }
    }

    /// Aggregate node embeddings into line embeddings
    /// nodes: [B, 19, D] -> lines: [B, 15, D]
    fn nodes_to_lines(&self, nodes: &Tensor) -> Tensor {
        let device = nodes.device();

        let mut line_embeds = Vec::with_capacity(15);
        for line_idx in 0..15usize {
            let len = LINE_LENGTHS[line_idx] as usize;
            let member_indices: Vec<i64> = LINE_MEMBERS[line_idx][..len].to_vec();
            let idx = Tensor::from_slice(&member_indices).to_device(device);

            // Gather + mean pool: [B, D]
            let pooled = nodes.index_select(1, &idx).mean_dim(1, false, Kind::Float);

            // Add direction type embedding (0-4 -> dir0, 5-9 -> dir1, 10-14 -> dir2)
            let dir = if line_idx < 5 { 0i64 } else if line_idx < 10 { 1 } else { 2 };
            let type_emb = self.line_type_embed.get(dir);

            // Add length embedding (3->0, 4->1, 5->2)
            let len_emb = self.line_len_embed.get(len as i64 - 3);

            line_embeds.push(pooled + type_emb.unsqueeze(0) + len_emb.unsqueeze(0));
        }

        Tensor::stack(&line_embeds, 1).apply(&self.line_proj)
    }

    /// Forward: nodes [B, 19, D] -> nodes [B, 19, D]
    fn forward(&self, nodes: &Tensor, train: bool, dropout: f64) -> Tensor {
        // 1. Node self-attention
        let normed = nodes.apply(&self.node_ln1);
        let attn_out = self.node_attn.forward(&normed, train, dropout);
        let attn_out = if train && dropout > 0.0 { attn_out.dropout(dropout, train) } else { attn_out };
        let nodes_sa = nodes + attn_out;

        // 2. Node -> Line aggregation
        let lines = self.nodes_to_lines(&nodes_sa);

        // 3. Line self-attention
        let lines_normed = lines.apply(&self.line_ln);
        let lines_attn = self.line_attn.forward(&lines_normed, train, dropout);
        let lines_attn = if train && dropout > 0.0 { lines_attn.dropout(dropout, train) } else { lines_attn };
        let lines_out = &lines + lines_attn;

        // 4. Line -> Node cross-attention (nodes query their relevant lines)
        let nodes_normed = nodes_sa.apply(&self.cross_ln);
        let cross_out = self.cross_attn.cross_forward(&nodes_normed, &lines_out, train, dropout);

        // 5. Gated fusion
        let gate_input = Tensor::cat(&[&nodes_sa, &cross_out], -1);
        let gate = gate_input.apply(&self.gate_proj).sigmoid();
        let fused = &nodes_sa + &gate * &cross_out;

        // 6. FFN
        let normed = fused.apply(&self.ffn_ln);
        let ffn_out = self.ffn.forward(&normed, train, dropout);
        let ffn_out = if train && dropout > 0.0 { ffn_out.dropout(dropout, train) } else { ffn_out };
        fused + ffn_out
    }
}

/// Hypergraph Transformer Policy Network
pub struct HypergraphTransformerPolicyNet {
    input_proj: nn::Linear,
    pos_embed: Tensor,
    layers: Vec<HyperLayer>,
    final_ln: nn::LayerNorm,
    policy_head: nn::Linear,
    dropout: f64,
}

impl HypergraphTransformerPolicyNet {
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
            layers.push(HyperLayer::new(
                &(&p / format!("hyper_{}", i)),
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

/// Hypergraph Transformer Value Network
pub struct HypergraphTransformerValueNet {
    input_proj: nn::Linear,
    pos_embed: Tensor,
    layers: Vec<HyperLayer>,
    final_ln: nn::LayerNorm,
    value_head: nn::Sequential,
    dropout: f64,
}

impl HypergraphTransformerValueNet {
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
            layers.push(HyperLayer::new(
                &(&p / format!("hyper_{}", i)),
                embed_dim, num_heads, ff_dim,
            ));
        }

        let final_ln = nn::layer_norm(
            &p / "final_ln",
            vec![embed_dim],
            nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
        );

        let value_head = nn::seq()
            .add(nn::linear(&p / "value_fc1", embed_dim, 64, Default::default()))
            .add_fn(|x| x.relu())
            .add(nn::linear(&p / "value_fc2", 64, 1, Default::default()));

        Self { input_proj, pos_embed, layers, final_ln, value_head, dropout }
    }

    /// node_features: [B, 19, input_dim] -> [B, 1] value in [-1, 1]
    pub fn forward(&self, node_features: &Tensor, train: bool) -> Tensor {
        let mut h = node_features.apply(&self.input_proj) + self.pos_embed.unsqueeze(0);
        if train && self.dropout > 0.0 {
            h = h.dropout(self.dropout, train);
        }

        for layer in &self.layers {
            h = layer.forward(&h, train, self.dropout);
        }

        h = h.apply(&self.final_ln);
        let pooled = h.mean_dim(1, false, Kind::Float);
        pooled.apply(&self.value_head).tanh()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hypergraph_policy_forward() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let net = HypergraphTransformerPolicyNet::new(&vs, 47, 128, 2, 4, 0.1);

        let x = Tensor::randn([4, 19, 47], (Kind::Float, tch::Device::Cpu));
        let logits = net.forward(&x, false);
        assert_eq!(logits.size(), vec![4, 19]);
    }

    #[test]
    fn test_hypergraph_value_forward() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let net = HypergraphTransformerValueNet::new(&vs, 47, 128, 2, 4, 0.1);

        let x = Tensor::randn([4, 19, 47], (Kind::Float, tch::Device::Cpu));
        let out = net.forward(&x, false);
        assert_eq!(out.size(), vec![4, 1]);
        let max_val: f64 = out.abs().max().double_value(&[]);
        assert!(max_val <= 1.0);
    }

    #[test]
    fn test_node_line_mapping_consistency() {
        // Each line's members should be valid node indices
        for (line_idx, members) in LINE_MEMBERS.iter().enumerate() {
            let len = LINE_LENGTHS[line_idx] as usize;
            for &node in &members[..len] {
                assert!(node >= 0 && node < 19, "Line {} has invalid node {}", line_idx, node);
            }
            // Padding should be -1
            for &pad in &members[len..] {
                assert_eq!(pad, -1, "Line {} padding should be -1", line_idx);
            }
        }
    }

    #[test]
    fn test_hyper_layer_forward() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let layer = HyperLayer::new(&vs.root(), 64, 4, 256);

        let nodes = Tensor::randn([2, 19, 64], (Kind::Float, tch::Device::Cpu));
        let out = layer.forward(&nodes, false, 0.0);
        assert_eq!(out.size(), vec![2, 19, 64]);
    }

    #[test]
    fn test_nodes_to_lines() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let layer = HyperLayer::new(&vs.root(), 64, 4, 256);

        let nodes = Tensor::randn([2, 19, 64], (Kind::Float, tch::Device::Cpu));
        let lines = layer.nodes_to_lines(&nodes);
        assert_eq!(lines.size(), vec![2, 15, 64]);
    }
}
