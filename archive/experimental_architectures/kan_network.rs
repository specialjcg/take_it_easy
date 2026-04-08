//! Kolmogorov-Arnold Network (KAN) for Take It Easy Policy Prediction
//!
//! A KAN replaces traditional linear layers (fixed activation, learnable weights)
//! with learnable activation functions on each edge of the network. Instead of
//! y = activation(Wx + b), each connection (i,j) has its own non-linear function
//! phi_ij, and the layer computes y_j = sum_i phi_ij(x_i).
//!
//! This is inspired by the Kolmogorov-Arnold representation theorem, which states
//! that any multivariate continuous function can be represented as a superposition
//! of continuous single-variable functions.
//!
//! Implementation details:
//! - Each edge activation is parameterized as a weighted sum of RBF basis functions
//!   (Gaussian kernels on a fixed grid), which is equivalent in expressivity to
//!   B-spline KAN but much simpler to implement with tensor ops on GPU.
//! - Each KANLinear computes: base_weight * silu(x) + spline_weight * rbf_spline(x)
//!   where rbf_spline uses learnable coefficients over Gaussian basis functions.
//! - The architecture follows the same input/output contract as other policy nets
//!   in this codebase: [B, 19, input_dim] -> [B, 19] logits.

use tch::{nn, Kind, Tensor};

const NODE_COUNT: i64 = 19;

/// A KAN linear layer: replaces nn::Linear with learnable per-edge activations.
///
/// For each (out, in) pair, the activation is:
///   phi(x) = base_weight * silu(x) + sum_k coeff_k * exp(-inv_sigma * (x - center_k)^2)
///
/// This gives each edge its own learnable non-linear transfer function.
struct KANLinear {
    base_weight: Tensor,   // [out_features, in_features]
    spline_coeffs: Tensor, // [out_features, in_features, grid_size]
    centers: Tensor,       // [grid_size] - fixed RBF centers
    inv_sigma: f64,        // 1 / (2 * sigma^2)
}

impl KANLinear {
    fn new(path: &nn::Path, in_features: i64, out_features: i64, grid_size: i64) -> Self {
        let base_weight = path.var(
            "base_weight",
            &[out_features, in_features],
            nn::Init::Randn {
                mean: 0.0,
                stdev: (2.0 / in_features as f64).sqrt(),
            },
        );

        // Initialize spline coefficients small so the network starts near a standard linear layer
        let spline_coeffs = path.var(
            "spline_coeffs",
            &[out_features, in_features, grid_size],
            nn::Init::Randn {
                mean: 0.0,
                stdev: 0.1 / (grid_size as f64).sqrt(),
            },
        );

        // Fixed grid centers uniformly spaced in [-2, 2]
        let centers = Tensor::linspace(-2.0, 2.0, grid_size, (Kind::Float, path.device()));

        // sigma chosen so adjacent basis functions overlap well
        let spacing = 4.0 / (grid_size as f64 - 1.0).max(1.0);
        let sigma = spacing; // sigma = grid spacing
        let inv_sigma = 1.0 / (2.0 * sigma * sigma);

        Self {
            base_weight,
            spline_coeffs,
            centers,
            inv_sigma,
        }
    }

    /// Forward pass: x [B, N, in_features] -> [B, N, out_features]
    fn forward(&self, x: &Tensor) -> Tensor {
        // Base path: standard linear on silu(x)
        // silu(x) = x * sigmoid(x)
        let base_input = x.silu(); // [B, N, in]
        // base_output = base_input @ base_weight^T -> [B, N, out]
        let base_output = base_input.matmul(&self.base_weight.tr());

        // Spline path: RBF basis functions
        // x: [B, N, in] -> expand to [B, N, in, 1]
        let x_expanded = x.unsqueeze(-1); // [B, N, in, 1]
        // centers: [grid_size] -> [1, 1, 1, grid_size]
        let centers = self.centers.reshape([1, 1, 1, -1]);
        // diff: [B, N, in, grid_size]
        let diff = &x_expanded - &centers;
        // RBF basis: exp(-inv_sigma * diff^2) -> [B, N, in, grid_size]
        let basis = (diff.square() * (-self.inv_sigma)).exp();

        // Compute spline output via einsum-style contraction:
        // spline_coeffs: [out, in, grid_size]
        // basis: [B, N, in, grid_size]
        // result: [B, N, out] = sum over in and grid_size
        //
        // Manual approach: reshape and matmul
        let (b, n, _in_f) = x.size3().unwrap();
        let in_features = self.base_weight.size()[1];
        let grid_size = self.centers.size()[0];
        let out_features = self.base_weight.size()[0];

        // basis: [B, N, in, grid] -> [B*N, in*grid]
        let basis_flat = basis.reshape([b * n, in_features * grid_size]);
        // spline_coeffs: [out, in, grid] -> [out, in*grid]
        let coeffs_flat = self.spline_coeffs.reshape([out_features, in_features * grid_size]);
        // spline_output: [B*N, out] = basis_flat @ coeffs_flat^T
        let spline_output = basis_flat.matmul(&coeffs_flat.tr());
        let spline_output = spline_output.reshape([b, n, out_features]);

        base_output + spline_output
    }
}

/// A KAN layer: KANLinear + LayerNorm + residual connection
struct KANLayer {
    kan_linear: KANLinear,
    ln: nn::LayerNorm,
    // Optional second KANLinear for FFN-style expansion
    kan_expand: KANLinear,
    kan_contract: KANLinear,
    ffn_ln: nn::LayerNorm,
}

impl KANLayer {
    fn new(path: &nn::Path, embed_dim: i64, ff_dim: i64, grid_size: i64) -> Self {
        Self {
            kan_linear: KANLinear::new(&(path / "kan"), embed_dim, embed_dim, grid_size),
            ln: nn::layer_norm(
                path / "ln",
                vec![embed_dim],
                nn::LayerNormConfig {
                    eps: 1e-5,
                    ..Default::default()
                },
            ),
            kan_expand: KANLinear::new(&(path / "kan_expand"), embed_dim, ff_dim, grid_size),
            kan_contract: KANLinear::new(&(path / "kan_contract"), ff_dim, embed_dim, grid_size),
            ffn_ln: nn::layer_norm(
                path / "ffn_ln",
                vec![embed_dim],
                nn::LayerNormConfig {
                    eps: 1e-5,
                    ..Default::default()
                },
            ),
        }
    }

    /// Forward: [B, N, D] -> [B, N, D] with residual
    fn forward(&self, x: &Tensor, train: bool, dropout: f64) -> Tensor {
        // Pre-norm KAN mixing
        let normed = x.apply(&self.ln);
        let h = self.kan_linear.forward(&normed);
        let h = if train && dropout > 0.0 {
            h.dropout(dropout, train)
        } else {
            h
        };
        let x = x + h;

        // Pre-norm KAN FFN (expand then contract)
        let normed = x.apply(&self.ffn_ln);
        let h = self.kan_expand.forward(&normed);
        let h = self.kan_contract.forward(&h);
        let h = if train && dropout > 0.0 {
            h.dropout(dropout, train)
        } else {
            h
        };
        x + h
    }
}

/// KAN Policy Network for Take It Easy
///
/// Architecture:
///   input_proj: Linear(input_dim -> embed_dim)
///   pos_embed: learnable [19, embed_dim]
///   N x KANLayer(embed_dim, ff_dim, grid_size)
///   final_ln: LayerNorm
///   policy_head: Linear(embed_dim -> 1)
///
/// Input: [B, 19, input_dim] -> Output: [B, 19] logits
pub struct KANPolicyNet {
    input_proj: nn::Linear,
    pos_embed: Tensor,
    layers: Vec<KANLayer>,
    final_ln: nn::LayerNorm,
    policy_head: nn::Linear,
    dropout: f64,
}

impl KANPolicyNet {
    pub fn new(
        vs: &nn::VarStore,
        input_dim: i64,
        embed_dim: i64,
        num_layers: usize,
        grid_size: i64,
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
            layers.push(KANLayer::new(
                &(&p / format!("kan_{}", i)),
                embed_dim,
                ff_dim,
                grid_size,
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
    fn test_kan_forward() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let net = KANPolicyNet::new(&vs, 47, 64, 2, 8, 0.1);
        let x = Tensor::randn([4, 19, 47], (Kind::Float, tch::Device::Cpu));
        let out = net.forward(&x, false);
        assert_eq!(out.size(), vec![4, 19]);
    }

    #[test]
    fn test_kan_param_count() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let _net = KANPolicyNet::new(&vs, 47, 128, 3, 8, 0.1);
        let count: i64 = vs.variables().values().map(|t| t.numel() as i64).sum();
        println!("KAN params: {}", count);
        assert!(count > 0);
    }

    #[test]
    fn test_kan_linear_shapes() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let kan = KANLinear::new(&vs.root(), 32, 64, 8);
        let x = Tensor::randn([2, 19, 32], (Kind::Float, tch::Device::Cpu));
        let out = kan.forward(&x);
        assert_eq!(out.size(), vec![2, 19, 64]);
    }

    #[test]
    fn test_kan_layer_residual() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let layer = KANLayer::new(&vs.root(), 64, 256, 8);
        let x = Tensor::randn([2, 19, 64], (Kind::Float, tch::Device::Cpu));
        let out = layer.forward(&x, false, 0.0);
        assert_eq!(out.size(), vec![2, 19, 64]);
    }

    #[test]
    fn test_kan_train_mode() {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let net = KANPolicyNet::new(&vs, 47, 64, 2, 8, 0.1);
        let x = Tensor::randn([4, 19, 47], (Kind::Float, tch::Device::Cpu));
        let out_train = net.forward(&x, true);
        let out_eval = net.forward(&x, false);
        assert_eq!(out_train.size(), vec![4, 19]);
        assert_eq!(out_eval.size(), vec![4, 19]);
    }
}
