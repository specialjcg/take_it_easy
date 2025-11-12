use crate::neural::tensor_conversion::{convert_plateau_to_graph_features, GRAPH_NODE_COUNT};
use tch::IndexOp;
use tch::{nn, Kind, Tensor};

// Silver GNN: Increased capacity for better representation learning
const DEFAULT_HIDDEN: &[i64] = &[128, 128, 64];

#[derive(Debug)]
pub struct GraphLayer {
    w_self: nn::Linear,
    w_neigh: nn::Linear,
    ln: nn::LayerNorm, // LayerNorm instead of BatchNorm for graph-friendly normalization
    use_residual: bool,
}

impl GraphLayer {
    pub fn new<'a>(path: &nn::Path<'a>, in_dim: i64, out_dim: i64) -> Self {
        let w_self = nn::linear(path / "self", in_dim, out_dim, Default::default());
        let w_neigh = nn::linear(path / "neigh", in_dim, out_dim, Default::default());
        // LayerNorm normalizes over feature dimension, preserving spatial structure
        let ln = nn::layer_norm(
            path / "ln",
            vec![out_dim],
            nn::LayerNormConfig {
                eps: 1e-5,
                ..Default::default()
            },
        );
        let use_residual = in_dim == out_dim;
        Self {
            w_self,
            w_neigh,
            ln,
            use_residual,
        }
    }

    pub fn forward(&self, x: &Tensor, adj: &Tensor, train: bool, dropout: f64) -> Tensor {
        let residual = if self.use_residual {
            Some(x.shallow_clone())
        } else {
            None
        };

        let self_term = x.apply(&self.w_self);
        let neigh_msg = x.apply(&self.w_neigh);
        let batch_size = x.size()[0];
        let adj_exp = adj.unsqueeze(0).expand([batch_size, -1, -1], false);
        let neigh_agg = adj_exp.bmm(&neigh_msg);

        // Combine self and neighbor information
        let mut out = self_term + neigh_agg;

        // Apply LayerNorm: normalizes over feature dimension [out_dim]
        // Input shape: [batch, nodes, features] - LayerNorm applies per node, over features
        // This preserves spatial structure (each node normalized independently)
        out = out.apply(&self.ln);

        // Add residual connection if dimensions match (before activation)
        if let Some(res) = residual {
            out = out + res;
        }

        // Activation
        out = out.relu();

        // Dropout
        if train && dropout > 0.0 {
            out = out.dropout(dropout, train);
        }

        out
    }
}

pub struct GraphEncoder {
    layers: Vec<GraphLayer>,
    adj_norm: Tensor,
    dropout: f64,
}

impl GraphEncoder {
    pub fn new(vs: &nn::VarStore, input_dim: i64, hidden_dims: &[i64], dropout: f64) -> Self {
        let p = vs.root();
        let mut layers = Vec::new();
        let mut dims = Vec::from(hidden_dims);
        if dims.is_empty() {
            dims.extend_from_slice(DEFAULT_HIDDEN);
        }

        let mut in_dim = input_dim;
        for (idx, &out_dim) in dims.iter().enumerate() {
            let layer =
                GraphLayer::new(&(p.clone() / format!("graph_layer_{idx}")), in_dim, out_dim);
            layers.push(layer);
            in_dim = out_dim;
        }

        let adj_norm = build_normalized_adjacency();

        Self {
            layers,
            adj_norm,
            dropout,
        }
    }

    pub fn forward(&self, x: &Tensor, train: bool) -> Tensor {
        let mut h = x.shallow_clone();
        for layer in &self.layers {
            h = layer.forward(&h, &self.adj_norm, train, self.dropout);
        }
        h
    }

    pub fn output_dim(&self) -> i64 {
        self.layers
            .last()
            .map(|layer| layer.w_self.ws.size()[0])
            .unwrap_or(0)
    }
}

fn build_normalized_adjacency() -> Tensor {
    let adj = Tensor::zeros(
        [GRAPH_NODE_COUNT as i64, GRAPH_NODE_COUNT as i64],
        (Kind::Float, tch::Device::Cpu),
    );
    for &(src, dst) in crate::neural::tensor_conversion::GRAPH_EDGES.iter() {
        let _ = adj.i((src as i64, dst as i64)).fill_(1.0);
        let _ = adj.i((dst as i64, src as i64)).fill_(1.0);
    }
    for idx in 0..GRAPH_NODE_COUNT {
        let _ = adj.i((idx as i64, idx as i64)).fill_(1.0);
    }
    // Normalisation symétrique : D^(-1/2) * A * D^(-1/2)
    let degree = adj.sum_dim_intlist([1].as_ref(), false, Kind::Float); // [19] sans keep_dim
    let degree_inv_sqrt = degree.pow_tensor_scalar(-0.5);
    let degree_inv_sqrt = degree_inv_sqrt.masked_fill(&degree_inv_sqrt.isinf(), 0.0);
    // Broadcasting : degree_inv_sqrt est [19], on le reshape en [19, 1] pour la multiplication à gauche
    let d_inv_sqrt_left = degree_inv_sqrt.unsqueeze(1); // [19, 1]
    let d_inv_sqrt_right = degree_inv_sqrt.unsqueeze(0); // [1, 19]
                                                         // Normalisation symétrique par broadcasting
    &adj * &d_inv_sqrt_left * &d_inv_sqrt_right
}

pub struct GraphPolicyNet {
    encoder: GraphEncoder,
    head: nn::Linear,
}

impl GraphPolicyNet {
    pub fn new(vs: &nn::VarStore, input_dim: i64, hidden_dims: &[i64], dropout: f64) -> Self {
        let encoder = GraphEncoder::new(vs, input_dim, hidden_dims, dropout);
        let output_dim = encoder.output_dim();
        let head = nn::linear(vs.root() / "policy_head", output_dim, 1, Default::default());
        Self { encoder, head }
    }

    pub fn forward(&self, node_features: &Tensor, train: bool) -> Tensor {
        let h = self.encoder.forward(node_features, train);
        let logits = h.apply(&self.head).squeeze_dim(-1);
        logits.softmax(-1, Kind::Float)
    }
}

pub struct GraphValueNet {
    encoder: GraphEncoder,
    head: nn::Linear,
}

impl GraphValueNet {
    pub fn new(vs: &nn::VarStore, input_dim: i64, hidden_dims: &[i64], dropout: f64) -> Self {
        let encoder = GraphEncoder::new(vs, input_dim, hidden_dims, dropout);
        let output_dim = encoder.output_dim();
        let head = nn::linear(vs.root() / "value_head", output_dim, 1, Default::default());
        Self { encoder, head }
    }

    pub fn forward(&self, node_features: &Tensor, train: bool) -> Tensor {
        let h = self.encoder.forward(node_features, train);
        let pooled = h.mean_dim(&[1i64][..], false, Kind::Float);
        pooled.apply(&self.head).tanh()
    }
}

pub fn convert_plateau_for_gnn(
    plateau: &crate::game::plateau::Plateau,
    current_turn: usize,
    total_turns: usize,
) -> Tensor {
    convert_plateau_to_graph_features(plateau, current_turn, total_turns)
}
