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
    /// Number of top ply-1 positions to keep in 3-ply pruning (default 3).
    pub top_k_ply1: usize,
    /// Number of top ply-2 positions to keep per (p1, t1) in 3-ply pruning (default 2).
    pub top_k_ply2: usize,
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

/// 2-ply expectimax returning EVs for all legal positions.
///
/// Returns `(best_pos, evs)` where `evs` is a Vec of `(position, expected_value)`.
/// For turns before min_turn or degenerate cases, returns a single-entry EV list.
pub fn expectimax_2ply_with_evs(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    turn: usize,
    policy_net: &GraphTransformerPolicyNet,
    value_net: &GraphTransformerValueNet,
    config: &ExpectimaxConfig,
) -> (usize, Vec<(usize, f64)>) {
    let legal = get_legal_moves(plateau);
    if legal.len() <= 1 {
        let pos = legal.first().copied().unwrap_or(0);
        return (pos, vec![(pos, 0.0)]);
    }

    // Before min_turn: GT Direct — return policy logits as proxy EVs
    if turn < config.min_turn {
        let pos = gt_direct_gpu(plateau, tile, deck, turn, policy_net, config);
        let evs: Vec<(usize, f64)> = legal.iter().map(|&p| (p, if p == pos { 1.0 } else { 0.0 })).collect();
        return (pos, evs);
    }
    // Last 2 turns: fall back to 1-ply EVs
    if turn >= 17 {
        return expectimax_1ply_with_evs(plateau, tile, deck, turn, policy_net, value_net, config);
    }

    let deck_after_1 = replace_tile_in_deck(deck, tile);
    let future_tiles_1 = get_available_tiles(&deck_after_1);
    if future_tiles_1.is_empty() {
        let pos = gt_direct_gpu(plateau, tile, deck, turn, policy_net, config);
        return (pos, vec![(pos, 0.0)]);
    }

    let n_p = legal.len();
    let n_t1 = future_tiles_1.len();
    let n_q = n_p - 1;

    if n_q == 0 {
        return expectimax_1ply_with_evs(plateau, tile, deck, turn, policy_net, value_net, config);
    }

    // Build all leaf features: [p][t1][q][t2]
    let mut all_features: Vec<Tensor> = Vec::new();
    let mut n_t2 = 0usize;

    for &pos in &legal {
        let mut plateau_1 = plateau.clone();
        plateau_1.tiles[pos] = *tile;
        let legal_2 = get_legal_moves(&plateau_1);

        for ft1 in &future_tiles_1 {
            let deck_after_2 = replace_tile_in_deck(&deck_after_1, ft1);
            let future_tiles_2 = get_available_tiles(&deck_after_2);
            if n_t2 == 0 { n_t2 = future_tiles_2.len(); }

            for &pos2 in &legal_2 {
                let mut plateau_2 = plateau_1.clone();
                plateau_2.tiles[pos2] = *ft1;
                for ft2 in &future_tiles_2 {
                    all_features.push(convert_plateau_for_gat_47ch(
                        &plateau_2, ft2, &deck_after_2, turn + 2, 19,
                    ));
                }
            }
        }
    }

    if n_t2 == 0 || all_features.is_empty() {
        return expectimax_1ply_with_evs(plateau, tile, deck, turn, policy_net, value_net, config);
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
        ).unwrap();
        all_values.extend(vals);
    }

    // Aggregate: [n_p][n_t1][n_q][n_t2] → EV per position
    let mut evs: Vec<(usize, f64)> = Vec::with_capacity(n_p);
    let mut best_pos = legal[0];
    let mut best_ev = f64::NEG_INFINITY;

    for (p_idx, &pos) in legal.iter().enumerate() {
        let mut ev_p = 0.0;
        for t1_idx in 0..n_t1 {
            let mut best_q_val = f64::NEG_INFINITY;
            for q_idx in 0..n_q {
                let base = ((p_idx * n_t1 + t1_idx) * n_q + q_idx) * n_t2;
                let avg_t2 = all_values[base..base + n_t2].iter().sum::<f64>() / n_t2 as f64;
                if avg_t2 > best_q_val { best_q_val = avg_t2; }
            }
            ev_p += best_q_val;
        }
        ev_p /= n_t1 as f64;
        evs.push((pos, ev_p));
        if ev_p > best_ev { best_ev = ev_p; best_pos = pos; }
    }

    (best_pos, evs)
}

/// 3-ply expectimax with progressive pruning.
///
/// Tree: max(p1) → avg(t1) → max(p2) → avg(t2) → max(p3) → avg(t3) → V(state)
///
/// Pruning strategy:
///   Phase 1: 1-ply screening → keep top-K₁ positions (reuses `expectimax_1ply_with_evs`)
///   Phase 2: For each top-K₁ p1, run 2-ply sub-trees → keep top-K₂ ply-2 positions per (p1, t1)
///   Phase 3: For each (p1, t1, top-K₂ p2), evaluate 3-ply leaves in streaming GPU batches
///
/// At turn 8 with K₁=3, K₂=2: ~274k leaf evals (~3s per move on GPU).
/// Falls back to 2-ply for turn 16, 1-ply for turn ≥ 17, GT Direct before min_turn.
pub fn expectimax_3ply_select(
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
    // Last 3 turns: fall back to 2-ply (not enough depth for 3-ply)
    if turn >= 16 {
        return expectimax_2ply_select(plateau, tile, deck, turn, policy_net, value_net, config);
    }

    let deck_after_1 = replace_tile_in_deck(deck, tile);
    let future_tiles_1 = get_available_tiles(&deck_after_1);
    if future_tiles_1.is_empty() {
        return gt_direct_gpu(plateau, tile, deck, turn, policy_net, config);
    }

    let n_p = legal.len();
    let n_t1 = future_tiles_1.len();
    let top_k1 = config.top_k_ply1.min(n_p);
    let top_k2 = config.top_k_ply2;

    if n_p <= 2 {
        // Too few positions for 3-ply pruning to help — use 2-ply
        return expectimax_2ply_select(plateau, tile, deck, turn, policy_net, value_net, config);
    }

    // ── Phase 1: 1-ply screening → top-K₁ positions ──
    let (_best_1ply, evs_1ply) =
        expectimax_1ply_with_evs(plateau, tile, deck, turn, policy_net, value_net, config);

    // Sort by EV descending, keep top-K₁
    let mut sorted_evs = evs_1ply.clone();
    sorted_evs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let top_positions: Vec<usize> = sorted_evs.iter().take(top_k1).map(|&(p, _)| p).collect();

    // ── Phase 2: 2-ply sub-trees for top-K₁ → select top-K₂ ply-2 positions per (p1, t1) ──
    // For each (p1, t1): build features for all (q, t2), forward, average over t2,
    // then keep the top-K₂ q positions.

    let chunk_size = 8192;

    // selected_p2[p1_idx][t1_idx] = Vec of (position_index_in_legal2, position) for top-K₂
    let mut selected_p2: Vec<Vec<Vec<(usize, usize)>>> = Vec::with_capacity(top_k1);
    // Also accumulate the 2-ply EV contribution per (p1, t1) for the selected positions
    // We'll recompute in phase 3 anyway, so we just need selected_p2 here.

    for &p1 in &top_positions {
        let mut plateau_1 = plateau.clone();
        plateau_1.tiles[p1] = *tile;
        let legal_2 = get_legal_moves(&plateau_1);
        let n_q = legal_2.len();

        let mut per_t1: Vec<Vec<(usize, usize)>> = Vec::with_capacity(n_t1);

        for ft1 in &future_tiles_1 {
            let deck_after_2 = replace_tile_in_deck(&deck_after_1, ft1);
            let future_tiles_2 = get_available_tiles(&deck_after_2);
            let n_t2 = future_tiles_2.len();

            if n_q == 0 || n_t2 == 0 {
                per_t1.push(Vec::new());
                continue;
            }

            let k2 = top_k2.min(n_q);

            // Build features: [n_q][n_t2]
            let mut features: Vec<Tensor> = Vec::with_capacity(n_q * n_t2);
            for &pos2 in &legal_2 {
                let mut plateau_2 = plateau_1.clone();
                plateau_2.tiles[pos2] = *ft1;
                for ft2 in &future_tiles_2 {
                    features.push(convert_plateau_for_gat_47ch(
                        &plateau_2, ft2, &deck_after_2, turn + 2, 19,
                    ));
                }
            }

            // Chunked forward pass
            let mut values: Vec<f64> = Vec::with_capacity(features.len());
            for chunk in features.chunks(chunk_size) {
                let batch = Tensor::stack(chunk, 0).to_device(config.device);
                let v = tch::no_grad(|| value_net.forward(&batch, false))
                    .to_device(Device::Cpu);
                let vals: Vec<f64> =
                    Vec::<f64>::try_from(&v.squeeze_dim(1).to_kind(Kind::Double)).unwrap();
                values.extend(vals);
            }

            // Aggregate: avg over t2 for each q
            let mut q_evs: Vec<(usize, usize, f64)> = Vec::with_capacity(n_q);
            for (q_idx, &pos2) in legal_2.iter().enumerate() {
                let base = q_idx * n_t2;
                let avg_t2 = values[base..base + n_t2].iter().sum::<f64>() / n_t2 as f64;
                q_evs.push((q_idx, pos2, avg_t2));
            }

            // Sort descending, keep top-K₂
            q_evs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
            let selected: Vec<(usize, usize)> =
                q_evs.iter().take(k2).map(|&(qi, p, _)| (qi, p)).collect();
            per_t1.push(selected);
        }

        selected_p2.push(per_t1);
    }

    // ── Phase 3: 3-ply leaves, streaming by (p1, t1, selected_p2) ──
    // For each (p1, t1, selected q): place ft1 at q, then for each t2 in future_tiles_2,
    // enumerate all legal_3 positions, for each t3 in future_tiles_3, evaluate V.
    // Aggregate: avg(t3) → max(p3) → accumulate.

    let mut best_pos = top_positions[0];
    let mut best_ev = f64::NEG_INFINITY;

    for (p1_idx, &p1) in top_positions.iter().enumerate() {
        let mut plateau_1 = plateau.clone();
        plateau_1.tiles[p1] = *tile;

        let mut ev_p1 = 0.0; // will accumulate avg over t1

        for (t1_idx, ft1) in future_tiles_1.iter().enumerate() {
            let deck_after_2 = replace_tile_in_deck(&deck_after_1, ft1);
            let future_tiles_2 = get_available_tiles(&deck_after_2);

            let selected = &selected_p2[p1_idx][t1_idx];
            if selected.is_empty() || future_tiles_2.is_empty() {
                // Degenerate: no ply-2 moves, use 2-ply value as fallback
                // (shouldn't happen in practice)
                continue;
            }

            let mut best_q_val = f64::NEG_INFINITY;

            for &(_q_idx, q_pos) in selected {
                let mut plateau_2 = plateau_1.clone();
                plateau_2.tiles[q_pos] = *ft1;
                let legal_3 = get_legal_moves(&plateau_2);
                let n_r = legal_3.len();

                if n_r == 0 {
                    // No moves at ply 3 — evaluate terminal
                    continue;
                }

                // For each t2: enumerate (r, t3) leaves
                let mut ev_q = 0.0; // accumulate avg over t2

                for ft2 in &future_tiles_2 {
                    let deck_after_3 = replace_tile_in_deck(&deck_after_2, ft2);
                    let future_tiles_3 = get_available_tiles(&deck_after_3);
                    let n_t3 = future_tiles_3.len();

                    if n_t3 == 0 {
                        // Last tile placed — no future tiles, evaluate each r directly
                        let mut features: Vec<Tensor> = Vec::with_capacity(n_r);
                        for &pos3 in &legal_3 {
                            let mut plateau_3 = plateau_2.clone();
                            plateau_3.tiles[pos3] = *ft2;
                            // Use a dummy tile/deck for terminal evaluation
                            features.push(convert_plateau_for_gat_47ch(
                                &plateau_3,
                                ft2,
                                &deck_after_3,
                                turn + 3,
                                19,
                            ));
                        }
                        let mut values: Vec<f64> = Vec::with_capacity(features.len());
                        for chunk in features.chunks(chunk_size) {
                            let batch = Tensor::stack(chunk, 0).to_device(config.device);
                            let v = tch::no_grad(|| value_net.forward(&batch, false))
                                .to_device(Device::Cpu);
                            let vals: Vec<f64> = Vec::<f64>::try_from(
                                &v.squeeze_dim(1).to_kind(Kind::Double),
                            )
                            .unwrap();
                            values.extend(vals);
                        }
                        let best_r = values
                            .iter()
                            .copied()
                            .fold(f64::NEG_INFINITY, f64::max);
                        ev_q += best_r;
                        continue;
                    }

                    // Build features: [n_r][n_t3]
                    let mut features: Vec<Tensor> = Vec::with_capacity(n_r * n_t3);
                    for &pos3 in &legal_3 {
                        let mut plateau_3 = plateau_2.clone();
                        plateau_3.tiles[pos3] = *ft2;
                        for ft3 in &future_tiles_3 {
                            features.push(convert_plateau_for_gat_47ch(
                                &plateau_3, ft3, &deck_after_3, turn + 3, 19,
                            ));
                        }
                    }

                    // Chunked forward pass
                    let mut values: Vec<f64> = Vec::with_capacity(features.len());
                    for chunk in features.chunks(chunk_size) {
                        let batch = Tensor::stack(chunk, 0).to_device(config.device);
                        let v = tch::no_grad(|| value_net.forward(&batch, false))
                            .to_device(Device::Cpu);
                        let vals: Vec<f64> = Vec::<f64>::try_from(
                            &v.squeeze_dim(1).to_kind(Kind::Double),
                        )
                        .unwrap();
                        values.extend(vals);
                    }

                    // Aggregate: avg(t3) → max(r)
                    let mut best_r = f64::NEG_INFINITY;
                    for r_idx in 0..n_r {
                        let base = r_idx * n_t3;
                        let avg_t3 =
                            values[base..base + n_t3].iter().sum::<f64>() / n_t3 as f64;
                        if avg_t3 > best_r {
                            best_r = avg_t3;
                        }
                    }

                    ev_q += best_r;
                }

                ev_q /= future_tiles_2.len() as f64; // avg over t2
                if ev_q > best_q_val {
                    best_q_val = ev_q;
                }
            }

            if best_q_val > f64::NEG_INFINITY {
                ev_p1 += best_q_val;
            }
        }

        ev_p1 /= n_t1 as f64; // avg over t1

        if ev_p1 > best_ev {
            best_ev = ev_p1;
            best_pos = p1;
        }
    }

    best_pos
}

/// 1-ply expectimax returning EVs for all legal positions.
fn expectimax_1ply_with_evs(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    turn: usize,
    policy_net: &GraphTransformerPolicyNet,
    value_net: &GraphTransformerValueNet,
    config: &ExpectimaxConfig,
) -> (usize, Vec<(usize, f64)>) {
    let legal = get_legal_moves(plateau);
    if legal.len() <= 1 {
        let pos = legal.first().copied().unwrap_or(0);
        return (pos, vec![(pos, 0.0)]);
    }
    if turn >= 18 {
        let pos = gt_direct_gpu(plateau, tile, deck, turn, policy_net, config);
        let evs = legal.iter().map(|&p| (p, if p == pos { 1.0 } else { 0.0 })).collect();
        return (pos, evs);
    }

    let deck_after = replace_tile_in_deck(deck, tile);
    let future_tiles = get_available_tiles(&deck_after);
    if future_tiles.is_empty() {
        let pos = gt_direct_gpu(plateau, tile, deck, turn, policy_net, config);
        return (pos, vec![(pos, 0.0)]);
    }

    let mut all_features: Vec<Tensor> = Vec::with_capacity(legal.len() * future_tiles.len());
    for &pos in &legal {
        let mut new_plateau = plateau.clone();
        new_plateau.tiles[pos] = *tile;
        for future_tile in &future_tiles {
            all_features.push(convert_plateau_for_gat_47ch(
                &new_plateau, future_tile, &deck_after, turn + 1, 19,
            ));
        }
    }

    let batch = Tensor::stack(&all_features, 0).to_device(config.device);
    let values = tch::no_grad(|| value_net.forward(&batch, false))
        .to_device(Device::Cpu);
    let values_flat: Vec<f64> = Vec::<f64>::try_from(
        &values.squeeze_dim(1).to_kind(Kind::Double),
    ).unwrap();

    let n_future = future_tiles.len();
    let mut evs: Vec<(usize, f64)> = Vec::with_capacity(legal.len());
    let mut best_pos = legal[0];
    let mut best_ev = f64::NEG_INFINITY;

    for (i, &pos) in legal.iter().enumerate() {
        let start = i * n_future;
        let ev: f64 = values_flat[start..start + n_future].iter().sum::<f64>() / n_future as f64;
        evs.push((pos, ev));
        if ev > best_ev { best_ev = ev; best_pos = pos; }
    }

    (best_pos, evs)
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
