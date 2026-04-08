//! Mamba (Selective State Space Model) for Take It Easy
//!
//! Instead of attention (O(N^2)) or sheaf diffusion, Mamba uses a selective
//! state space model with input-dependent parameters. The key insight:
//! the SSM's hidden state accumulates information along a scan direction,
//! letting each position "see" all previous positions in that direction.
//!
//! For the hex board, we scan along the 3 scoring directions (columns, diag/, diag\).
//! Each direction defines a specific ordering of the 19 positions.
//! Bidirectional scans (forward + backward) ensure every node sees its entire line.
//!
//! Architecture per layer:
//!   1. For each direction d (0, 1, 2):
//!      a. Permute nodes to direction's scan order
//!      b. Conv1d (local context, kernel=4)
//!      c. Selective SSM scan (forward + backward)
//!      d. Gated output
//!      e. Unpermute back to original order
//!   2. Fuse 3 directions via learned projection
//!   3. FFN + residual
//!
//! Properties:
//!   - Linear complexity O(N) per direction (vs O(N^2) for attention)
//!   - Input-dependent selectivity: learns WHAT to remember per position
//!   - Direction-aware: separate scans per scoring axis
//!   - Sequential inductive bias: natural for turn-by-turn game progression

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

/// Inverse permutations to restore original node order after scanning
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

/// Selective SSM core
///
/// Implements the discretized state space model:
///   dt = softplus(dt_proj(x))
///   A_bar = exp(A * dt)
///   B_bar = dt * B(x)
///   h_t = A_bar * h_{t-1} + B_bar * x_t
///   y_t = C(x_t) * h_t + D * x_t
struct SelectiveSSM {
    dt_proj: nn::Linear,  // d_inner -> d_state
    b_proj: nn::Linear,   // d_inner -> d_state
    c_proj: nn::Linear,   // d_inner -> d_state
    a_log: Tensor,        // [d_inner, d_state] - log-space for stability
    d_param: Tensor,      // [d_inner] skip connection
    d_inner: i64,
    d_state: i64,
}

impl SelectiveSSM {
    fn new(path: &nn::Path, d_inner: i64, d_state: i64) -> Self {
        let dt_proj = nn::linear(
            path / "dt_proj",
            d_inner,
            d_state,
            nn::LinearConfig { bias: true, ..Default::default() },
        );
        let b_proj = nn::linear(path / "b_proj", d_inner, d_state, Default::default());
        let c_proj = nn::linear(path / "c_proj", d_inner, d_state, Default::default());

        // Initialize A in log-space: A = -exp(a_log), ensures A < 0 (stable)
        // Standard init: A_log[i,j] = log(1 + j) for each i
        // A in log-space, init ~1.0 (log(e)=1). Exact init per-column would need
        // mutable access; uniform init is fine as training will adjust.
        let a_log = (path / "a_log").var(
            "weight",
            &[d_inner, d_state],
            nn::Init::Randn { mean: 1.0, stdev: 0.1 },
        );

        let d_param = (path / "d_param").var(
            "weight",
            &[d_inner],
            nn::Init::Const(1.0),
        );

        Self { dt_proj, b_proj, c_proj, a_log, d_param, d_inner, d_state }
    }

    /// Run selective scan over a sequence
    /// x: [B, L, D_inner] -> [B, L, D_inner]
    fn scan(&self, x: &Tensor, reverse: bool) -> Tensor {
        let (b, l, _d) = x.size3().unwrap();
        let device = x.device();

        // Compute input-dependent parameters
        let dt = x.apply(&self.dt_proj).softplus(); // [B, L, D_state]
        let b_param = x.apply(&self.b_proj);        // [B, L, D_state]
        let c_param = x.apply(&self.c_proj);        // [B, L, D_state]

        // A = -exp(a_log) -> always negative (stable)
        let a = -self.a_log.exp(); // [D_inner, D_state]

        // Sequential scan
        let mut h = Tensor::zeros([b, self.d_inner, self.d_state], (Kind::Float, device));
        let mut outputs: Vec<Tensor> = Vec::with_capacity(l as usize);

        let indices: Vec<i64> = if reverse {
            (0..l).rev().collect()
        } else {
            (0..l).collect()
        };

        for &t in &indices {
            let x_t = x.select(1, t);          // [B, D_inner]
            let dt_t = dt.select(1, t);         // [B, D_state]
            let b_t = b_param.select(1, t);     // [B, D_state]
            let c_t = c_param.select(1, t);     // [B, D_state]

            // Discretize: A_bar = exp(A * dt)
            // A: [D_inner, D_state], dt_t: [B, D_state]
            // We need A_bar: [B, D_inner, D_state]
            let a_dt = &a.unsqueeze(0) * &dt_t.unsqueeze(1); // [B, D_inner, D_state]
            let a_bar = a_dt.exp();

            // B_bar = (dt * B(x)) * x_t: [B, D_inner, D_state]
            // dt_t: [B, D_state], b_t: [B, D_state], x_t: [B, D_inner]
            let b_bar = (&dt_t.unsqueeze(1) * &b_t.unsqueeze(1)) * &x_t.unsqueeze(2);

            // h = A_bar * h + B_bar
            h = &a_bar * &h + &b_bar;

            // y_t = (C_t * h).sum(d_state) + D * x_t
            // c_t: [B, D_state] -> [B, 1, D_state]
            let y_t = (&h * &c_t.unsqueeze(1)).sum_dim_intlist(-1, false, Kind::Float);
            // y_t: [B, D_inner]
            let y_t = &y_t + &x_t * &self.d_param;

            outputs.push(y_t.unsqueeze(1)); // [B, 1, D_inner]
        }

        // If reversed, we need to reverse outputs back
        if reverse {
            outputs.reverse();
        }

        Tensor::cat(&outputs, 1) // [B, L, D_inner]
    }
}

/// One Mamba block for a single scan direction
struct MambaBlock {
    in_proj: nn::Linear,   // embed_dim -> 2 * d_inner (split into x' and gate z)
    conv_weight: nn::Linear, // Simple 1D "conv" via grouped linear
    ssm: SelectiveSSM,
    out_proj: nn::Linear,  // d_inner -> embed_dim
    ln: nn::LayerNorm,
    d_inner: i64,
}

impl MambaBlock {
    fn new(path: &nn::Path, embed_dim: i64, d_inner: i64, d_state: i64) -> Self {
        Self {
            in_proj: nn::linear(path / "in_proj", embed_dim, d_inner * 2, Default::default()),
            conv_weight: nn::linear(path / "conv", d_inner, d_inner, Default::default()),
            ssm: SelectiveSSM::new(&(path / "ssm"), d_inner, d_state),
            out_proj: nn::linear(path / "out_proj", d_inner, embed_dim, Default::default()),
            ln: nn::layer_norm(
                path / "ln",
                vec![embed_dim],
                nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
            ),
            d_inner,
        }
    }

    /// Forward pass with bidirectional scan
    /// x: [B, 19, embed_dim] -> [B, 19, embed_dim]
    /// scan_order: permutation to apply before scanning
    /// inv_order: inverse permutation to restore original order
    fn forward(
        &self,
        x: &Tensor,
        scan_order: &Tensor,
        inv_order: &Tensor,
        train: bool,
        dropout: f64,
    ) -> Tensor {
        let normed = x.apply(&self.ln);

        // Permute to scan order: [B, 19, D]
        let permuted = normed.index_select(1, scan_order);

        // Project to 2*d_inner and split
        let proj = permuted.apply(&self.in_proj); // [B, 19, 2*d_inner]
        let xz = proj.chunk(2, -1);
        let x_in = xz[0].silu(); // [B, 19, d_inner]
        let z = &xz[1]; // [B, 19, d_inner] gate

        // Simple "conv" for local context
        let x_conv = x_in.apply(&self.conv_weight).silu();

        // Bidirectional SSM scan
        let y_fwd = self.ssm.scan(&x_conv, false);
        let y_bwd = self.ssm.scan(&x_conv, true);
        let y = y_fwd + y_bwd;

        // Gate
        let y = &y * z.sigmoid();
        let y = if train && dropout > 0.0 {
            y.dropout(dropout, train)
        } else {
            y
        };

        // Project back and unpermute
        let out = y.apply(&self.out_proj); // [B, 19, embed_dim]
        out.index_select(1, inv_order)
    }
}

/// One Mamba layer: 3 directional scans + fusion + FFN
struct MambaLayer {
    blocks: Vec<MambaBlock>,  // [3] one per direction
    fusion_proj: nn::Linear,  // 3*embed_dim -> embed_dim
    ffn_ln: nn::LayerNorm,
    ffn: FFN,
}

impl MambaLayer {
    fn new(path: &nn::Path, embed_dim: i64, d_inner: i64, d_state: i64, ff_dim: i64) -> Self {
        let mut blocks = Vec::with_capacity(NUM_DIRECTIONS);
        for d in 0..NUM_DIRECTIONS {
            blocks.push(MambaBlock::new(
                &(path / format!("mamba_{}", d)),
                embed_dim,
                d_inner,
                d_state,
            ));
        }

        Self {
            blocks,
            fusion_proj: nn::linear(
                path / "fusion",
                embed_dim * 3,
                embed_dim,
                Default::default(),
            ),
            ffn_ln: nn::layer_norm(
                path / "ffn_ln",
                vec![embed_dim],
                nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
            ),
            ffn: FFN::new(&(path / "ffn"), embed_dim, ff_dim),
        }
    }

    fn forward(
        &self,
        x: &Tensor,
        scan_orders: &[Tensor; 3],
        inv_orders: &[Tensor; 3],
        train: bool,
        dropout: f64,
    ) -> Tensor {
        // Run 3 directional Mamba blocks
        let msgs: Vec<Tensor> = (0..NUM_DIRECTIONS)
            .map(|d| self.blocks[d].forward(x, &scan_orders[d], &inv_orders[d], train, dropout))
            .collect();

        // Fuse: concat + project
        let fused = Tensor::cat(&[&msgs[0], &msgs[1], &msgs[2]], -1);
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

/// Mamba Policy Net for Take It Easy
pub struct MambaPolicyNet {
    input_proj: nn::Linear,
    pos_embed: Tensor,
    layers: Vec<MambaLayer>,
    final_ln: nn::LayerNorm,
    policy_head: nn::Linear,
    dropout: f64,
    // Pre-computed scan order tensors (moved to device on first forward)
    scan_orders: [Tensor; 3],
    inv_orders: [Tensor; 3],
}

impl MambaPolicyNet {
    pub fn new(
        vs: &nn::VarStore,
        input_dim: i64,
        embed_dim: i64,
        d_state: i64,
        num_layers: usize,
        dropout: f64,
    ) -> Self {
        let p = vs.root();
        let d_inner = embed_dim * 2; // Mamba standard: expand by 2x
        let ff_dim = embed_dim * 4;

        let input_proj = nn::linear(&p / "input_proj", input_dim, embed_dim, Default::default());
        let pos_embed = (&p / "pos_embed").var(
            "weight",
            &[NODE_COUNT, embed_dim],
            nn::Init::Randn { mean: 0.0, stdev: 0.02 },
        );

        let mut layers = Vec::with_capacity(num_layers);
        for i in 0..num_layers {
            layers.push(MambaLayer::new(
                &(&p / format!("mamba_layer_{}", i)),
                embed_dim,
                d_inner,
                d_state,
                ff_dim,
            ));
        }

        let final_ln = nn::layer_norm(
            &p / "final_ln",
            vec![embed_dim],
            nn::LayerNormConfig { eps: 1e-5, ..Default::default() },
        );
        let policy_head = nn::linear(&p / "policy_head", embed_dim, 1, Default::default());

        // Pre-compute scan orders
        let device = vs.device();
        let scan_orders = std::array::from_fn(|d| {
            let order: Vec<i64> = SCAN_ORDERS[d].iter().map(|&i| i as i64).collect();
            Tensor::from_slice(&order).to_device(device)
        });
        let inv_orders = std::array::from_fn(|d| {
            let inv = inverse_perm(&SCAN_ORDERS[d]);
            let order: Vec<i64> = inv.iter().map(|&i| i as i64).collect();
            Tensor::from_slice(&order).to_device(device)
        });

        Self {
            input_proj,
            pos_embed,
            layers,
            final_ln,
            policy_head,
            dropout,
            scan_orders,
            inv_orders,
        }
    }

    /// node_features: [B, 19, input_dim] -> [B, 19] logits
    pub fn forward(&self, node_features: &Tensor, train: bool) -> Tensor {
        let mut h = node_features.apply(&self.input_proj) + self.pos_embed.unsqueeze(0);
        if train && self.dropout > 0.0 {
            h = h.dropout(self.dropout, train);
        }

        for layer in &self.layers {
            h = layer.forward(&h, &self.scan_orders, &self.inv_orders, train, self.dropout);
        }

        h = h.apply(&self.final_ln);
        h.apply(&self.policy_head).squeeze_dim(-1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mamba_policy_forward() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let net = MambaPolicyNet::new(&vs, 47, 128, 16, 2, 0.1);

        let x = Tensor::randn([4, 19, 47], (Kind::Float, tch::Device::Cpu));
        let logits = net.forward(&x, false);
        assert_eq!(logits.size(), vec![4, 19]);
    }

    #[test]
    fn test_mamba_policy_train_mode() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let net = MambaPolicyNet::new(&vs, 47, 64, 16, 2, 0.1);

        let x = Tensor::randn([2, 19, 47], (Kind::Float, tch::Device::Cpu));
        let logits = net.forward(&x, true);
        assert_eq!(logits.size(), vec![2, 19]);
    }

    #[test]
    fn test_selective_ssm_scan() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let ssm = SelectiveSSM::new(&vs.root(), 64, 16);

        let x = Tensor::randn([2, 19, 64], (Kind::Float, tch::Device::Cpu));
        let y_fwd = ssm.scan(&x, false);
        let y_bwd = ssm.scan(&x, true);
        assert_eq!(y_fwd.size(), vec![2, 19, 64]);
        assert_eq!(y_bwd.size(), vec![2, 19, 64]);
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
            assert_eq!(sorted.to_vec(), expected, "Direction {} is not a permutation", d);
        }
    }

    #[test]
    fn test_param_count() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let _net = MambaPolicyNet::new(&vs, 47, 128, 16, 3, 0.1);
        let count: i64 = vs.variables().values().map(|t| t.numel() as i64).sum();
        println!("Mamba param count (dim=128, d_state=16, 3 layers): {}", count);
        assert!(count > 0);
    }
}
