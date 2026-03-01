//! Expectimax strategy using a value network for 1-ply lookahead.
//!
//! For each legal position, places the tile, then averages V(state') over all
//! possible future tiles. Zero rollouts, zero noise — purely deterministic
//! evaluation. GPU-batched: ~300 evals in one forward pass.

use tch::{Device, Kind, Tensor};

use crate::game::deck::Deck;
use crate::game::get_legal_moves::get_legal_moves;
use crate::game::plateau::Plateau;
use crate::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use crate::game::tile::Tile;
use crate::neural::graph_transformer::{GraphTransformerPolicyNet, GraphTransformerValueNet};
use crate::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use crate::strategy::gt_boost::line_boost;

pub struct ExpectimaxConfig {
    pub device: Device,
    pub boost: f64,
    pub score_mean: f64,
    pub score_std: f64,
    /// Minimum turn to activate expectimax (before this, use GT Direct).
    /// Set to 0 to always use expectimax.
    pub min_turn: usize,
}

/// 1-ply expectimax: pick the position that maximises E[V | place tile at p].
///
/// Falls back to GT Direct for the last turn (no future tiles to average over).
pub fn expectimax_select(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    turn: usize,
    policy_net: &GraphTransformerPolicyNet,
    value_net: &GraphTransformerValueNet,
    config: &ExpectimaxConfig,
) -> usize {
    let legal = get_legal_moves(plateau);
    if legal.len() <= 1 {
        return legal.first().copied().unwrap_or(0);
    }

    // Before min_turn or last turn: use GT Direct
    if turn < config.min_turn || turn >= 18 {
        return gt_direct_gpu(plateau, tile, deck, turn, policy_net, config);
    }

    let deck_after = replace_tile_in_deck(deck, tile);
    let future_tiles = get_available_tiles(&deck_after);

    if future_tiles.is_empty() {
        return gt_direct_gpu(plateau, tile, deck, turn, policy_net, config);
    }

    // Build batched features: for each (legal_pos, future_tile) pair
    let mut all_features: Vec<Tensor> = Vec::with_capacity(legal.len() * future_tiles.len());

    for &pos in &legal {
        let mut new_plateau = plateau.clone();
        new_plateau.tiles[pos] = *tile;

        for future_tile in &future_tiles {
            let feat = convert_plateau_for_gat_47ch(
                &new_plateau,
                future_tile,
                &deck_after,
                turn + 1,
                19,
            );
            all_features.push(feat);
        }
    }

    // Single batched forward pass: [n_legal * n_future, 19, 47] -> [n_legal * n_future, 1]
    let batch = Tensor::stack(&all_features, 0).to_device(config.device);
    let values = tch::no_grad(|| value_net.forward(&batch, false))
        .to_device(Device::Cpu); // [n, 1]

    // Denormalize: tanh output in [-1, 1] -> real score
    let values_flat: Vec<f64> = Vec::<f64>::try_from(
        &values.squeeze_dim(1).to_kind(Kind::Double),
    )
    .unwrap();

    // Average V over future tiles for each legal position
    let n_future = future_tiles.len();
    let mut best_pos = legal[0];
    let mut best_ev = f64::NEG_INFINITY;

    for (i, &pos) in legal.iter().enumerate() {
        let start = i * n_future;
        let end = start + n_future;
        let ev: f64 = values_flat[start..end].iter().sum::<f64>() / n_future as f64;

        if ev > best_ev {
            best_ev = ev;
            best_pos = pos;
        }
    }

    best_pos
}

/// GPU-aware GT Direct fallback: logits + line_boost → argmax.
fn gt_direct_gpu(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    turn: usize,
    policy_net: &GraphTransformerPolicyNet,
    config: &ExpectimaxConfig,
) -> usize {
    let legal = get_legal_moves(plateau);
    if legal.len() <= 1 {
        return legal.first().copied().unwrap_or(0);
    }

    let feat = convert_plateau_for_gat_47ch(plateau, tile, deck, turn, 19)
        .unsqueeze(0)
        .to_device(config.device);
    let logits = tch::no_grad(|| policy_net.forward(&feat, false))
        .squeeze_dim(0)
        .to_device(Device::Cpu);
    let logit_values: Vec<f64> = Vec::<f64>::try_from(&logits).unwrap();

    *legal
        .iter()
        .max_by(|&&a, &&b| {
            let sa = logit_values[a] + line_boost(plateau, tile, a, config.boost);
            let sb = logit_values[b] + line_boost(plateau, tile, b, config.boost);
            sa.partial_cmp(&sb).unwrap()
        })
        .unwrap()
}
