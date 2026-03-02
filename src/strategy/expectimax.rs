//! Expectimax strategy using a value network for lookahead.
//!
//! - **1-ply**: For each legal position, places the tile, then averages V(state')
//!   over all possible future tiles. ~300 evals per move, <2ms GPU.
//! - **2-ply**: Looks 2 moves ahead: max over p1, avg over tile1, max over p2,
//!   avg over tile2. ~30k evals per move at turn 8, chunked forward passes.

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

/// 2-ply expectimax: looks 2 moves ahead.
///
/// Tree structure: max(p1) → avg(tile1) → max(p2) → avg(tile2) → V(state)
///
/// At turn 8: ~11×18×10×17 ≈ 34k leaf evaluations, chunked into GPU batches.
/// Falls back to 1-ply for the second-to-last turn, GT Direct before min_turn.
pub fn expectimax_2ply_select(
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

    // Before min_turn: GT Direct
    if turn < config.min_turn {
        return gt_direct_gpu(plateau, tile, deck, turn, policy_net, config);
    }
    // Last 2 turns: fall back to 1-ply (not enough depth for 2-ply)
    if turn >= 17 {
        return expectimax_select(plateau, tile, deck, turn, policy_net, value_net, config);
    }

    let deck_after_1 = replace_tile_in_deck(deck, tile);
    let future_tiles_1 = get_available_tiles(&deck_after_1);
    if future_tiles_1.is_empty() {
        return gt_direct_gpu(plateau, tile, deck, turn, policy_net, config);
    }

    let n_p = legal.len();
    let n_t1 = future_tiles_1.len();
    let n_q = n_p - 1; // one position gets filled at ply 1

    if n_q == 0 {
        // Only 1 legal position left after ply 1 — 2-ply degenerates to 1-ply
        return expectimax_select(plateau, tile, deck, turn, policy_net, value_net, config);
    }

    // Collect all leaf features in order: [p][t1][q][t2]
    // The structure is rectangular because:
    // - |legal_2| = |legal| - 1 for every p (one position filled)
    // - |future_tiles_2| = |future_tiles_1| - 1 for every t1 (one tile used)
    let mut all_features: Vec<Tensor> = Vec::new();
    // Track n_t2 (should be uniform = n_t1 - 1)
    let mut n_t2 = 0usize;

    for &pos in &legal {
        let mut plateau_1 = plateau.clone();
        plateau_1.tiles[pos] = *tile;
        let legal_2 = get_legal_moves(&plateau_1);

        for ft1 in &future_tiles_1 {
            let deck_after_2 = replace_tile_in_deck(&deck_after_1, ft1);
            let future_tiles_2 = get_available_tiles(&deck_after_2);
            if n_t2 == 0 {
                n_t2 = future_tiles_2.len();
            }

            for &pos2 in &legal_2 {
                let mut plateau_2 = plateau_1.clone();
                plateau_2.tiles[pos2] = *ft1;

                for ft2 in &future_tiles_2 {
                    let feat = convert_plateau_for_gat_47ch(
                        &plateau_2, ft2, &deck_after_2, turn + 2, 19,
                    );
                    all_features.push(feat);
                }
            }
        }
    }

    if n_t2 == 0 || all_features.is_empty() {
        return expectimax_select(plateau, tile, deck, turn, policy_net, value_net, config);
    }

    // Chunked GPU forward pass
    let chunk_size = 8192;
    let mut all_values: Vec<f64> = Vec::with_capacity(all_features.len());

    for chunk in all_features.chunks(chunk_size) {
        let batch = Tensor::stack(chunk, 0).to_device(config.device);
        let values = tch::no_grad(|| value_net.forward(&batch, false))
            .to_device(Device::Cpu);
        let vals: Vec<f64> = Vec::<f64>::try_from(
            &values.squeeze_dim(1).to_kind(Kind::Double),
        )
        .unwrap();
        all_values.extend(vals);
    }

    // Aggregate: layout is [n_p][n_t1][n_q][n_t2] in row-major order
    // Step 1: avg over t2 → V(p, t1, q)
    // Step 2: max over q  → best_V(p, t1)   (player picks best ply-2 position)
    // Step 3: avg over t1 → E[V|p]           (nature picks random tile)
    let mut best_pos = legal[0];
    let mut best_ev = f64::NEG_INFINITY;

    for (p_idx, &_pos) in legal.iter().enumerate() {
        let mut ev_p = 0.0;

        for t1_idx in 0..n_t1 {
            let mut best_q_val = f64::NEG_INFINITY;

            for q_idx in 0..n_q {
                let base = ((p_idx * n_t1 + t1_idx) * n_q + q_idx) * n_t2;
                let avg_t2: f64 =
                    all_values[base..base + n_t2].iter().sum::<f64>() / n_t2 as f64;
                if avg_t2 > best_q_val {
                    best_q_val = avg_t2;
                }
            }

            ev_p += best_q_val;
        }

        ev_p /= n_t1 as f64;

        if ev_p > best_ev {
            best_ev = ev_p;
            best_pos = legal[p_idx];
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
