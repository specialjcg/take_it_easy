//! Perceiver Policy Network for Take It Easy
//!
//! The Perceiver architecture uses a small learned latent array as an information
//! bottleneck between the 19 board positions and the policy output. Instead of
//! full O(N^2) self-attention over positions, it uses:
//!
//!   1. **Encode**: Cross-attention from latents (query) to positions (key/value).
//!      The latent array "reads" relevant information from the board.
//!   2. **Process**: Self-attention on the latent array. The compressed
//!      representation is refined through interactions between latent tokens.
//!   3. **Decode**: Cross-attention from positions (query) to latents (key/value).
//!      Each position retrieves its policy-relevant information from the latents.
//!
//! Why this could work for the hex board:
//!   - The bottleneck (e.g. 8 latents for 19 positions) forces the network to
//!     compress the board state into a small number of abstract "concepts" —
//!     potentially discovering line completions, tile distributions, or
//!     strategic patterns without explicit topology.
//!   - Decoding back to per-position logits reconstructs a policy that is
//!     informed by the global compressed state, not just local features.
//!   - Computation scales as O(N*L) where L << N, though for N=19 this is
//!     more about representational inductive bias than efficiency.

use tch::{nn, Kind, Tensor};

const NODE_COUNT: i64 = 19;

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

/// Cross-attention: queries from one space, keys+values from another
struct CrossAttention {
    q_proj: nn::Linear,
    kv_proj: nn::Linear,
    out_proj: nn::Linear,
    num_heads: i64,
    head_dim: i64,
}

impl CrossAttention {
    fn new(path: &nn::Path, embed_dim: i64, num_heads: i64) -> Self {
        let head_dim = embed_dim / num_heads;
        Self {
            q_proj: nn::linear(path / "q", embed_dim, embed_dim, Default::default()),
            kv_proj: nn::linear(path / "kv", embed_dim, embed_dim * 2, Default::default()),
            out_proj: nn::linear(path / "out", embed_dim, embed_dim, Default::default()),
            num_heads,
            head_dim,
        }
    }

    /// q_input: [B, Nq, D], kv_input: [B, Nkv, D] -> [B, Nq, D]
    fn forward(&self, q_input: &Tensor, kv_input: &Tensor, train: bool, dropout: f64) -> Tensor {
        let (b, nq, _) = q_input.size3().unwrap();
        let (_, nkv, _) = kv_input.size3().unwrap();

        // Project queries
        let q = q_input.apply(&self.q_proj); // [B, Nq, D]
        let q = q
            .reshape([b, nq, self.num_heads, self.head_dim])
            .permute([0, 2, 1, 3]); // [B, H, Nq, Hd]

        // Project keys and values together
        let kv = kv_input.apply(&self.kv_proj); // [B, Nkv, 2*D]
        let kv = kv.reshape([b, nkv, 2, self.num_heads, self.head_dim]);
        let kv = kv.permute([2, 0, 3, 1, 4]); // [2, B, H, Nkv, Hd]
        let k = kv.select(0, 0); // [B, H, Nkv, Hd]
        let v = kv.select(0, 1); // [B, H, Nkv, Hd]

        // Scaled dot-product attention
        let scale = (self.head_dim as f64).sqrt();
        let attn = q.matmul(&k.transpose(-2, -1)) / scale; // [B, H, Nq, Nkv]
        let attn = attn.softmax(-1, Kind::Float);
        let attn = if train && dropout > 0.0 {
            attn.dropout(dropout, train)
        } else {
            attn
        };

        let out = attn.matmul(&v); // [B, H, Nq, Hd]
        let out = out.permute([0, 2, 1, 3]).reshape([b, nq, -1]); // [B, Nq, D]
        out.apply(&self.out_proj)
    }
}

/// Multi-head self-attention on the latent array
struct SelfAttention {
    qkv: nn::Linear,
    out_proj: nn::Linear,
    num_heads: i64,
    head_dim: i64,
}

impl SelfAttention {
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
        let (b, n, _) = x.size3().unwrap();
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

/// One Perceiver layer: encode (cross-attn) -> process (self-attn) -> FFN
struct PerceiverLayer {
    // Encode: latents attend to input positions
    encode_ln_q: nn::LayerNorm,
    encode_ln_kv: nn::LayerNorm,
    encode_attn: CrossAttention,

    // Process: self-attention on latent space
    self_ln: nn::LayerNorm,
    self_attn: SelfAttention,

    // FFN on latents
    ffn_ln: nn::LayerNorm,
    ffn: FFN,
}

impl PerceiverLayer {
    fn new(path: &nn::Path, embed_dim: i64, num_heads: i64) -> Self {
        let ff_dim = embed_dim * 4;
        let ln_config = nn::LayerNormConfig {
            eps: 1e-5,
            ..Default::default()
        };

        Self {
            encode_ln_q: nn::layer_norm(
                path / "enc_ln_q",
                vec![embed_dim],
                ln_config,
            ),
            encode_ln_kv: nn::layer_norm(
                path / "enc_ln_kv",
                vec![embed_dim],
                ln_config,
            ),
            encode_attn: CrossAttention::new(&(path / "enc_attn"), embed_dim, num_heads),
            self_ln: nn::layer_norm(
                path / "self_ln",
                vec![embed_dim],
                ln_config,
            ),
            self_attn: SelfAttention::new(&(path / "self_attn"), embed_dim, num_heads),
            ffn_ln: nn::layer_norm(
                path / "ffn_ln",
                vec![embed_dim],
                ln_config,
            ),
            ffn: FFN::new(&(path / "ffn"), embed_dim, ff_dim),
        }
    }

    /// latents: [B, L, D], x: [B, 19, D] -> latents: [B, L, D]
    fn forward(
        &self,
        latents: &Tensor,
        x: &Tensor,
        train: bool,
        dropout: f64,
    ) -> Tensor {
        // Encode: latents attend to input positions
        let q = latents.apply(&self.encode_ln_q);
        let kv = x.apply(&self.encode_ln_kv);
        let enc_out = self.encode_attn.forward(&q, &kv, train, dropout);
        let enc_out = if train && dropout > 0.0 {
            enc_out.dropout(dropout, train)
        } else {
            enc_out
        };
        let latents = latents + enc_out;

        // Process: self-attention on latent space
        let normed = latents.apply(&self.self_ln);
        let self_out = self.self_attn.forward(&normed, train, dropout);
        let self_out = if train && dropout > 0.0 {
            self_out.dropout(dropout, train)
        } else {
            self_out
        };
        let latents = &latents + self_out;

        // FFN
        let normed = latents.apply(&self.ffn_ln);
        let ffn_out = self.ffn.forward(&normed, train, dropout);
        let ffn_out = if train && dropout > 0.0 {
            ffn_out.dropout(dropout, train)
        } else {
            ffn_out
        };
        latents + ffn_out
    }
}

/// Perceiver Policy Network
///
/// Uses cross-attention bottleneck through a small latent array to
/// compress the 19-position board state and decode per-position policy logits.
pub struct PerceiverPolicyNet {
    input_proj: nn::Linear,
    pos_embed: Tensor,
    latent_array: Tensor,
    layers: Vec<PerceiverLayer>,

    // Decode: positions attend to latents
    decode_ln_q: nn::LayerNorm,
    decode_ln_kv: nn::LayerNorm,
    decode_attn: CrossAttention,

    final_ln: nn::LayerNorm,
    policy_head: nn::Linear,
    dropout: f64,
}

impl PerceiverPolicyNet {
    pub fn new(
        vs: &nn::VarStore,
        input_dim: i64,
        embed_dim: i64,
        num_latents: i64,
        num_layers: usize,
        num_heads: i64,
        dropout: f64,
    ) -> Self {
        let p = vs.root();
        let ln_config = nn::LayerNormConfig {
            eps: 1e-5,
            ..Default::default()
        };

        let input_proj = nn::linear(
            &p / "input_proj",
            input_dim,
            embed_dim,
            Default::default(),
        );
        let pos_embed = (&p / "pos_embed").var(
            "weight",
            &[NODE_COUNT, embed_dim],
            nn::Init::Randn {
                mean: 0.0,
                stdev: 0.02,
            },
        );
        let latent_array = (&p / "latent_array").var(
            "weight",
            &[num_latents, embed_dim],
            nn::Init::Randn {
                mean: 0.0,
                stdev: 0.02,
            },
        );

        let mut layers = Vec::with_capacity(num_layers);
        for i in 0..num_layers {
            layers.push(PerceiverLayer::new(
                &(&p / format!("perceiver_{}", i)),
                embed_dim,
                num_heads,
            ));
        }

        let decode_ln_q = nn::layer_norm(
            &p / "dec_ln_q",
            vec![embed_dim],
            ln_config,
        );
        let decode_ln_kv = nn::layer_norm(
            &p / "dec_ln_kv",
            vec![embed_dim],
            ln_config,
        );
        let decode_attn = CrossAttention::new(&(&p / "dec_attn"), embed_dim, num_heads);

        let final_ln = nn::layer_norm(
            &p / "final_ln",
            vec![embed_dim],
            ln_config,
        );
        let policy_head = nn::linear(
            &p / "policy_head",
            embed_dim,
            1,
            Default::default(),
        );

        Self {
            input_proj,
            pos_embed,
            latent_array,
            layers,
            decode_ln_q,
            decode_ln_kv,
            decode_attn,
            final_ln,
            policy_head,
            dropout,
        }
    }

    /// node_features: [B, 19, input_dim] -> [B, 19] logits
    pub fn forward(&self, node_features: &Tensor, train: bool) -> Tensor {
        let b = node_features.size()[0];

        // Project input and add positional embedding
        let x = node_features.apply(&self.input_proj) + self.pos_embed.unsqueeze(0); // [B, 19, D]
        let x = if train && self.dropout > 0.0 {
            x.dropout(self.dropout, train)
        } else {
            x
        };

        // Expand learned latent array to batch: [B, num_latents, D]
        let mut latents = self.latent_array.unsqueeze(0).expand([b, -1, -1], false);

        // Perceiver layers: encode + process
        for layer in &self.layers {
            latents = layer.forward(&latents, &x, train, self.dropout);
        }

        // Decode: positions attend to processed latents
        let q = x.apply(&self.decode_ln_q);
        let kv = latents.apply(&self.decode_ln_kv);
        let dec_out = self.decode_attn.forward(&q, &kv, train, self.dropout);
        let dec_out = if train && self.dropout > 0.0 {
            dec_out.dropout(self.dropout, train)
        } else {
            dec_out
        };
        let out = x + dec_out; // [B, 19, D]

        // Final projection to logits
        let out = out.apply(&self.final_ln);
        out.apply(&self.policy_head).squeeze_dim(-1) // [B, 19]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perceiver_forward() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let net = PerceiverPolicyNet::new(&vs, 47, 64, 8, 2, 4, 0.1);
        let x = Tensor::randn([4, 19, 47], (Kind::Float, tch::Device::Cpu));
        let out = net.forward(&x, false);
        assert_eq!(out.size(), vec![4, 19]);
    }

    #[test]
    fn test_perceiver_param_count() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let _net = PerceiverPolicyNet::new(&vs, 47, 128, 8, 3, 4, 0.1);
        let count: i64 = vs.variables().values().map(|t| t.numel() as i64).sum();
        println!("Perceiver params: {}", count);
        assert!(count > 0);
    }
}
