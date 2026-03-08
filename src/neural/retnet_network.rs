//! Retentive Network (RetNet) for Take It Easy
//!
//! RetNet replaces softmax attention with a multi-scale retention mechanism.
//! The key idea: Retention(Q, K, V) = (Q K^T * D) V, where D is a decay matrix
//! that weights interactions by distance. Unlike attention's uniform weighting,
//! retention has an exponential decay: D[i,j] = gamma^|i-j|, where gamma is a
//! per-head decay rate from the RetNet paper (gamma_h = 1 - 2^(-5 - h)).
//!
//! For the hex board, we use multi-directional retention: one retention module
//! per scoring direction (columns, diag/, diag\). Each direction defines a
//! specific ordering of the 19 positions via scan orders. Positions are permuted
//! to the scan order before retention and unpermuted after, so the decay matrix
//! reflects distance along scoring lines.
//!
//! We use BIDIRECTIONAL (non-causal) retention since all board positions are
//! visible simultaneously. The decay matrix uses absolute distance: D[i,j] = gamma^|i-j|.
//!
//! Architecture per layer:
//!   1. For each direction d (0=column, 1=diag/, 2=diag\):
//!      a. Permute nodes to direction's scan order
//!      b. Multi-scale retention with per-head decay rates
//!      c. Unpermute back to original order
//!   2. Fuse 3 directions via learned projection + residual
//!   3. FFN + residual
//!
//! Properties:
//!   - O(N^2) in parallel mode (same as attention) but with structural decay bias
//!   - No softmax: retention weights are deterministic from position, not learned
//!   - Multi-scale: different heads use different decay rates (local vs global)
//!   - Direction-aware: separate retention per scoring axis

use tch::{nn, Kind, Tensor};

const NODE_COUNT: i64 = 19;
const NUM_DIRECTIONS: usize = 3;

/// Scan orders: how to traverse the 19 positions for each scoring direction.
/// Each direction concatenates its 5 lines in order.
const SCAN_ORDERS: [[usize; 19]; 3] = [
    // Direction 0 (columns, top to bottom)
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18],
    // Direction 1 (diagonal /)
    [0, 3, 7, 1, 4, 8, 12, 2, 5, 9, 13, 16, 6, 10, 14, 17, 11, 15, 18],
    // Direction 2 (diagonal \)
    [7, 12, 16, 3, 8, 13, 17, 0, 4, 9, 14, 18, 1, 5, 10, 15, 2, 6, 11],
];

/// Inverse permutation to restore original node order after scanning
fn inverse_perm(perm: &[usize; 19]) -> [usize; 19] {
    let mut inv = [0usize; 19];
    for (i, &p) in perm.iter().enumerate() {
        inv[p] = i;
    }
    inv
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

/// Multi-Scale Retention mechanism
///
/// Implements parallel retention: Ret(Q, K, V) = (Q K^T * D) V
/// where D[i,j] = gamma^|i-j| is a bidirectional decay matrix.
/// Each head uses a different gamma (decay rate) for multi-scale modeling.
struct MultiScaleRetention {
    q_proj: nn::Linear,
    k_proj: nn::Linear,
    v_proj: nn::Linear,
    out_proj: nn::Linear,
    group_norm: nn::GroupNorm,
    num_heads: i64,
    head_dim: i64,
    /// Pre-computed decay matrices per head: [num_heads, 19, 19]
    decay_matrix: Tensor,
}

impl MultiScaleRetention {
    fn new(path: &nn::Path, embed_dim: i64, num_heads: i64, device: tch::Device) -> Self {
        let head_dim = embed_dim / num_heads;

        let q_proj = nn::linear(path / "q_proj", embed_dim, embed_dim, Default::default());
        let k_proj = nn::linear(path / "k_proj", embed_dim, embed_dim, Default::default());
        let v_proj = nn::linear(path / "v_proj", embed_dim, embed_dim, Default::default());
        let out_proj = nn::linear(path / "out_proj", embed_dim, embed_dim, Default::default());

        // GroupNorm: one group per head, applied over head_dim channels
        let group_norm = nn::group_norm(
            path / "gn",
            num_heads,
            embed_dim,
            nn::GroupNormConfig {
                eps: 1e-5,
                ..Default::default()
            },
        );

        // Pre-compute bidirectional decay matrix D[h, i, j] = gamma_h^|i-j|
        // gamma_h = 1 - 2^(-5 - h) for head h (from RetNet paper)
        let n = NODE_COUNT;
        let decay_matrix = Tensor::zeros([num_heads, n, n], (Kind::Float, device));
        for h in 0..num_heads {
            let gamma = 1.0 - f64::powf(2.0, -5.0 - h as f64);
            let mut d_data = vec![0.0f32; (n * n) as usize];
            for i in 0..n {
                for j in 0..n {
                    let dist = (i - j).unsigned_abs();
                    d_data[(i * n + j) as usize] = f64::powf(gamma, dist as f64) as f32;
                }
            }
            let d_h = Tensor::from_slice(&d_data)
                .reshape([n, n])
                .to_device(device);
            let _ = decay_matrix.select(0, h).copy_(&d_h);
        }

        Self {
            q_proj,
            k_proj,
            v_proj,
            out_proj,
            group_norm,
            num_heads,
            head_dim,
            decay_matrix,
        }
    }

    /// Forward pass: x [B, N, D] -> [B, N, D]
    fn forward(&self, x: &Tensor, train: bool, dropout: f64) -> Tensor {
        let (b, n, _d) = x.size3().unwrap();

        // Project Q, K, V
        let q = x.apply(&self.q_proj); // [B, N, D]
        let k = x.apply(&self.k_proj);
        let v = x.apply(&self.v_proj);

        // Reshape to multi-head: [B, H, N, Hd]
        let q = q
            .reshape([b, n, self.num_heads, self.head_dim])
            .permute([0, 2, 1, 3]);
        let k = k
            .reshape([b, n, self.num_heads, self.head_dim])
            .permute([0, 2, 1, 3]);
        let v = v
            .reshape([b, n, self.num_heads, self.head_dim])
            .permute([0, 2, 1, 3]);

        // Retention: (Q @ K^T) * D @ V
        // Q @ K^T: [B, H, N, N]
        let scale = (self.head_dim as f64).sqrt();
        let qk = q.matmul(&k.transpose(-2, -1)) / scale;

        // Apply decay: element-wise multiply with D [H, N, N] broadcast over B
        // decay_matrix: [H, N, N] -> [1, H, N, N]
        let qk = &qk * self.decay_matrix.unsqueeze(0);

        // Retention weights -> values: [B, H, N, Hd]
        let retention = qk.matmul(&v);

        // Group norm per head: reshape to [B, D, N] for GroupNorm, then back
        // retention: [B, H, N, Hd] -> [B, N, H, Hd] -> [B, N, D] -> [B, D, N]
        let retention = retention
            .permute([0, 2, 1, 3])
            .contiguous()
            .reshape([b, n, -1]); // [B, N, D]
        let retention = retention.permute([0, 2, 1]); // [B, D, N]
        let retention = retention.apply(&self.group_norm);
        let retention = retention.permute([0, 2, 1]); // [B, N, D]

        let retention = if train && dropout > 0.0 {
            retention.dropout(dropout, train)
        } else {
            retention
        };

        // Output projection
        retention.apply(&self.out_proj)
    }
}

/// One Retention Layer: multi-directional retention + fusion + FFN
struct RetentionLayer {
    /// One retention module per scoring direction
    retentions: Vec<MultiScaleRetention>,

    /// Direction fusion
    dir_ln: nn::LayerNorm,
    fusion_proj: nn::Linear,

    /// FFN
    ffn_ln: nn::LayerNorm,
    ffn: FFN,
}

impl RetentionLayer {
    fn new(
        path: &nn::Path,
        embed_dim: i64,
        num_heads: i64,
        ff_dim: i64,
        device: tch::Device,
    ) -> Self {
        let mut retentions = Vec::with_capacity(NUM_DIRECTIONS);
        for d in 0..NUM_DIRECTIONS {
            retentions.push(MultiScaleRetention::new(
                &(path / format!("ret_{}", d)),
                embed_dim,
                num_heads,
                device,
            ));
        }

        Self {
            retentions,
            dir_ln: nn::layer_norm(
                path / "dir_ln",
                vec![embed_dim],
                nn::LayerNormConfig {
                    eps: 1e-5,
                    ..Default::default()
                },
            ),
            fusion_proj: nn::linear(
                path / "fusion",
                embed_dim * 3,
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

    fn forward(
        &self,
        x: &Tensor,
        scan_orders: &[Tensor],
        inv_scan_orders: &[Tensor],
        train: bool,
        dropout: f64,
    ) -> Tensor {
        let normed = x.apply(&self.dir_ln);

        // Run retention per direction with scan-order permutation
        let mut dir_outs = Vec::with_capacity(NUM_DIRECTIONS);
        for d in 0..NUM_DIRECTIONS {
            // Permute to scan order
            let permuted = normed.index_select(1, &scan_orders[d]);
            // Apply retention
            let ret_out = self.retentions[d].forward(&permuted, train, dropout);
            // Unpermute back
            let unpermuted = ret_out.index_select(1, &inv_scan_orders[d]);
            dir_outs.push(unpermuted);
        }

        // Fuse: concat + project
        let fused = Tensor::cat(&[&dir_outs[0], &dir_outs[1], &dir_outs[2]], -1);
        let fused = fused.apply(&self.fusion_proj);

        // Residual
        let x = x + fused;
        let x = if train && dropout > 0.0 {
            x.dropout(dropout, train)
        } else {
            x
        };

        // FFN + residual
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

/// RetNet Policy Net for Take It Easy
///
/// Multi-directional retention with exponential decay captures
/// position-dependent relationships along the 3 scoring directions
/// of the hex board.
pub struct RetNetPolicyNet {
    input_proj: nn::Linear,
    pos_embed: Tensor,
    layers: Vec<RetentionLayer>,
    final_ln: nn::LayerNorm,
    policy_head: nn::Linear,
    dropout: f64,
    /// Pre-computed scan order tensors for each direction
    scan_orders: Vec<Tensor>,
    /// Pre-computed inverse scan order tensors for each direction
    inv_scan_orders: Vec<Tensor>,
}

impl RetNetPolicyNet {
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
        let device = vs.device();

        let input_proj =
            nn::linear(&p / "input_proj", input_dim, embed_dim, Default::default());
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
            layers.push(RetentionLayer::new(
                &(&p / format!("ret_layer_{}", i)),
                embed_dim,
                num_heads,
                ff_dim,
                device,
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
        let policy_head =
            nn::linear(&p / "policy_head", embed_dim, 1, Default::default());

        // Pre-compute scan orders
        let scan_orders: Vec<Tensor> = (0..NUM_DIRECTIONS)
            .map(|d| {
                let order: Vec<i64> = SCAN_ORDERS[d].iter().map(|&i| i as i64).collect();
                Tensor::from_slice(&order).to_device(device)
            })
            .collect();
        let inv_scan_orders: Vec<Tensor> = (0..NUM_DIRECTIONS)
            .map(|d| {
                let inv = inverse_perm(&SCAN_ORDERS[d]);
                let order: Vec<i64> = inv.iter().map(|&i| i as i64).collect();
                Tensor::from_slice(&order).to_device(device)
            })
            .collect();

        Self {
            input_proj,
            pos_embed,
            layers,
            final_ln,
            policy_head,
            dropout,
            scan_orders,
            inv_scan_orders,
        }
    }

    /// node_features: [B, 19, input_dim] -> [B, 19] logits
    pub fn forward(&self, node_features: &Tensor, train: bool) -> Tensor {
        let mut h = node_features.apply(&self.input_proj) + self.pos_embed.unsqueeze(0);
        if train && self.dropout > 0.0 {
            h = h.dropout(self.dropout, train);
        }

        for layer in &self.layers {
            h = layer.forward(
                &h,
                &self.scan_orders,
                &self.inv_scan_orders,
                train,
                self.dropout,
            );
        }

        h = h.apply(&self.final_ln);
        h.apply(&self.policy_head).squeeze_dim(-1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retnet_forward() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let net = RetNetPolicyNet::new(&vs, 47, 64, 2, 4, 0.1);
        let x = Tensor::randn([4, 19, 47], (Kind::Float, tch::Device::Cpu));
        let out = net.forward(&x, false);
        assert_eq!(out.size(), vec![4, 19]);
    }

    #[test]
    fn test_retnet_forward_train() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let net = RetNetPolicyNet::new(&vs, 47, 64, 2, 4, 0.1);
        let x = Tensor::randn([2, 19, 47], (Kind::Float, tch::Device::Cpu));
        let out = net.forward(&x, true);
        assert_eq!(out.size(), vec![2, 19]);
    }

    #[test]
    fn test_retnet_param_count() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let _net = RetNetPolicyNet::new(&vs, 47, 128, 3, 4, 0.1);
        let count: i64 = vs.variables().values().map(|t| t.numel() as i64).sum();
        println!("RetNet params: {}", count);
        assert!(count > 0);
    }

    #[test]
    fn test_decay_matrix_symmetry() {
        // Bidirectional decay matrix should be symmetric: D[i,j] = D[j,i]
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let ret = MultiScaleRetention::new(&vs.root(), 64, 4, tch::Device::Cpu);
        let d = &ret.decay_matrix; // [H, N, N]
        let d_t = d.transpose(-2, -1);
        let diff = (d - &d_t).abs().max();
        assert!(
            f64::try_from(&diff).unwrap() < 1e-6,
            "Decay matrix should be symmetric"
        );
    }

    #[test]
    fn test_decay_matrix_diagonal_ones() {
        // D[i,i] = gamma^0 = 1.0 for all heads
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let ret = MultiScaleRetention::new(&vs.root(), 64, 4, tch::Device::Cpu);
        for h in 0..4 {
            let d_h = ret.decay_matrix.select(0, h); // [N, N]
            let diag = d_h.diag(0); // [N]
            let ones = Tensor::ones([NODE_COUNT], (Kind::Float, tch::Device::Cpu));
            let diff = (&diag - &ones).abs().max();
            assert!(
                f64::try_from(&diff).unwrap() < 1e-6,
                "Diagonal of decay matrix for head {} should be 1.0",
                h
            );
        }
    }

    #[test]
    fn test_scan_order_inversions() {
        for d in 0..3 {
            let inv = inverse_perm(&SCAN_ORDERS[d]);
            for i in 0..19 {
                assert_eq!(inv[SCAN_ORDERS[d][i]], i);
            }
        }
    }

    #[test]
    fn test_scan_orders_are_permutations() {
        for d in 0..3 {
            let mut sorted = SCAN_ORDERS[d];
            sorted.sort();
            let expected: Vec<usize> = (0..19).collect();
            assert_eq!(
                sorted.to_vec(),
                expected,
                "Direction {} is not a permutation",
                d
            );
        }
    }
}
