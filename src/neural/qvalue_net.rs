//! Q-Value Network for position ranking
//!
//! Trained to predict softmax distribution over positions for action pruning.
//! Used in combination with CNN policy/value networks to improve MCTS.

use tch::{nn, Device, Kind, Tensor};
use crate::game::tile::Tile;

/// Q-Value Network architecture (matches training)
pub struct QValueNet {
    conv1: nn::Conv2D,
    bn1: nn::BatchNorm,
    conv2: nn::Conv2D,
    bn2: nn::BatchNorm,
    conv3: nn::Conv2D,
    bn3: nn::BatchNorm,
    fc1: nn::Linear,
    fc2: nn::Linear,
    qvalue_head: nn::Linear,
}

impl QValueNet {
    pub fn new(vs: &nn::VarStore) -> Self {
        let p = vs.root();

        let conv1 = nn::conv2d(&p / "conv1", 47, 64, 3, nn::ConvConfig { padding: 1, ..Default::default() });
        let bn1 = nn::batch_norm2d(&p / "bn1", 64, Default::default());

        let conv2 = nn::conv2d(&p / "conv2", 64, 128, 3, nn::ConvConfig { padding: 1, ..Default::default() });
        let bn2 = nn::batch_norm2d(&p / "bn2", 128, Default::default());

        let conv3 = nn::conv2d(&p / "conv3", 128, 128, 3, nn::ConvConfig { padding: 1, ..Default::default() });
        let bn3 = nn::batch_norm2d(&p / "bn3", 128, Default::default());

        let fc1 = nn::linear(&p / "fc1", 128 * 5 * 5, 512, Default::default());
        let fc2 = nn::linear(&p / "fc2", 512, 256, Default::default());
        let qvalue_head = nn::linear(&p / "qvalue_head", 256, 19, Default::default());

        Self { conv1, bn1, conv2, bn2, conv3, bn3, fc1, fc2, qvalue_head }
    }

    pub fn forward(&self, x: &Tensor) -> Tensor {
        let h = x.apply(&self.conv1).apply_t(&self.bn1, false).relu();
        let h = h.apply(&self.conv2).apply_t(&self.bn2, false).relu();
        let h = h.apply(&self.conv3).apply_t(&self.bn3, false).relu();
        let h = h.flat_view();
        let h = h.apply(&self.fc1).relu();
        let h = h.apply(&self.fc2).relu();
        h.apply(&self.qvalue_head)
    }

    /// Predict ranking probabilities for all 19 positions (softmax output)
    pub fn predict_ranking(&self, plateau: &[Tile], tile: &Tile) -> [f64; 19] {
        let state = encode_state(plateau, tile);
        let input = Tensor::from_slice(&state)
            .view([1, 47, 5, 5])
            .to_kind(Kind::Float);

        let output = self.forward(&input);
        let output_softmax = output.softmax(-1, Kind::Float);

        let mut probs = [0.0f64; 19];
        #[allow(clippy::needless_range_loop)]
        for i in 0..19 {
            probs[i] = output_softmax.double_value(&[0, i as i64]);
        }
        probs
    }

    /// Get top-K positions by ranking probability
    pub fn get_top_positions(&self, plateau: &[Tile], tile: &Tile, top_k: usize) -> Vec<usize> {
        let probs = self.predict_ranking(plateau, tile);

        // Filter to empty positions only
        let mut scored: Vec<(usize, f64)> = plateau.iter()
            .enumerate()
            .filter(|(_, t)| **t == Tile(0, 0, 0))
            .map(|(pos, _)| (pos, probs[pos]))
            .collect();

        // Sort by probability descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Return top-K positions
        scored.iter()
            .take(top_k.min(scored.len()))
            .map(|(pos, _)| *pos)
            .collect()
    }
}

/// Hexagonal position to grid index mapping
const HEX_TO_GRID: [(usize, usize); 19] = [
    (1, 0), (2, 0), (3, 0),
    (0, 1), (1, 1), (2, 1), (3, 1),
    (0, 2), (1, 2), (2, 2), (3, 2), (4, 2),
    (0, 3), (1, 3), (2, 3), (3, 3),
    (1, 4), (2, 4), (3, 4),
];

fn hex_to_grid_idx(hex_pos: usize) -> usize {
    let (row, col) = HEX_TO_GRID[hex_pos];
    row * 5 + col
}

fn encode_state(plateau: &[Tile], tile: &Tile) -> Vec<f32> {
    let mut state = vec![0.0f32; 47 * 5 * 5];

    let num_placed = plateau.iter().filter(|t| **t != Tile(0, 0, 0)).count();
    let turn_progress = num_placed as f32 / 19.0;

    for (hex_pos, t) in plateau.iter().enumerate() {
        let grid_idx = hex_to_grid_idx(hex_pos);

        if *t == Tile(0, 0, 0) {
            state[3 * 25 + grid_idx] = 1.0;
        } else {
            state[grid_idx] = t.0 as f32 / 9.0;
            state[25 + grid_idx] = t.1 as f32 / 9.0;
            state[2 * 25 + grid_idx] = t.2 as f32 / 9.0;
        }

        state[4 * 25 + grid_idx] = tile.0 as f32 / 9.0;
        state[5 * 25 + grid_idx] = tile.1 as f32 / 9.0;
        state[6 * 25 + grid_idx] = tile.2 as f32 / 9.0;
        state[7 * 25 + grid_idx] = turn_progress;
    }

    state
}

/// Q-Net manager for loading and caching
pub struct QNetManager {
    #[allow(dead_code)] // VarStore must stay alive to keep weights loaded
    vs: nn::VarStore,
    net: QValueNet,
}

impl QNetManager {
    pub fn new(model_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut vs = nn::VarStore::new(Device::Cpu);
        let net = QValueNet::new(&vs);
        vs.load(model_path)?;
        Ok(Self { vs, net })
    }

    pub fn net(&self) -> &QValueNet {
        &self.net
    }

    /// Consume self and return the QValueNet for Arc wrapping
    pub fn into_net(self) -> QValueNet {
        self.net
    }
}
