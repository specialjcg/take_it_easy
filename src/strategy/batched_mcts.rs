//! Batched MCTS with GPU-friendly inference.
//!
//! Key difference from `gt_boost::gt_mcts_select`: rollouts are grouped into
//! batches so that each turn uses a single forward pass of shape `[N, 19, 47]`
//! instead of N individual `[1, 19, 47]` calls. On GPU this is 10-50x faster.

use rand::prelude::*;
use rand::rngs::StdRng;
use tch::{Device, Tensor};

use crate::game::deck::Deck;
use crate::game::get_legal_moves::get_legal_moves;
use crate::game::plateau::Plateau;
use crate::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use crate::game::tile::Tile;
use crate::neural::graph_transformer::GraphTransformerPolicyNet;
use crate::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use crate::scoring::scoring::result;
use crate::strategy::gt_boost::line_boost;

/// Configuration for batched MCTS.
pub struct BatchedMctsConfig {
    /// Number of PUCT simulations.
    pub num_sims: usize,
    /// Line-boost strength (logit bonus).
    pub boost: f64,
    /// PUCT exploration constant.
    pub c_puct: f64,
    /// Device for neural network inference.
    pub device: Device,
    /// How many rollouts to batch together per forward pass.
    pub rollout_batch_size: usize,
}

impl Default for BatchedMctsConfig {
    fn default() -> Self {
        Self {
            num_sims: 100,
            boost: 3.0,
            c_puct: 2.5,
            device: Device::Cpu,
            rollout_batch_size: 32,
        }
    }
}

const SCORE_NORM: f64 = 300.0;
/// Neutral virtual loss value: keeps Q ≈ 0.5 during selection.
const VIRTUAL_LOSS_VALUE: f64 = 0.5 * SCORE_NORM;

/// PUCT MCTS with batched rollouts for GPU efficiency.
///
/// Same algorithm as `gt_mcts_select` but rollouts are batched: multiple
/// simulations share a single forward pass per turn.
pub fn batched_gt_mcts_select(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    turn: usize,
    policy_net: &GraphTransformerPolicyNet,
    config: &BatchedMctsConfig,
    rng: &mut StdRng,
) -> usize {
    let legal = get_legal_moves(plateau);
    if legal.len() <= 1 {
        return legal.first().copied().unwrap_or(0);
    }
    if turn >= 18 {
        // Last turn: just use GT+Boost argmax (no rollouts needed)
        return gt_boosted_select_on_device(plateau, tile, deck, turn, policy_net, config);
    }

    // Compute GT+Boost prior: softmax(gt_logits + line_boost)
    let priors = compute_prior(plateau, tile, deck, turn, policy_net, &legal, config);

    let n = legal.len();
    let mut visit_counts = vec![0usize; n];
    let mut total_values = vec![0.0f64; n];

    // Process simulations in batches
    let mut sims_done = 0;
    while sims_done < config.num_sims {
        let batch_size = (config.num_sims - sims_done).min(config.rollout_batch_size);

        // PUCT selection for each sim in the batch (with virtual loss for diversity)
        let mut batch_actions: Vec<usize> = Vec::with_capacity(batch_size);
        let mut batch_plateaus: Vec<Plateau> = Vec::with_capacity(batch_size);
        let mut batch_decks: Vec<Deck> = Vec::with_capacity(batch_size);

        for _ in 0..batch_size {
            let total_visits: usize = visit_counts.iter().sum();
            let sqrt_total = (total_visits as f64).sqrt();

            let best_idx = (0..n)
                .max_by(|&a, &b| {
                    let q_a = if visit_counts[a] > 0 {
                        total_values[a] / (visit_counts[a] as f64 * SCORE_NORM)
                    } else {
                        0.5
                    };
                    let q_b = if visit_counts[b] > 0 {
                        total_values[b] / (visit_counts[b] as f64 * SCORE_NORM)
                    } else {
                        0.5
                    };
                    let u_a = config.c_puct * priors[a] * sqrt_total / (1.0 + visit_counts[a] as f64);
                    let u_b = config.c_puct * priors[b] * sqrt_total / (1.0 + visit_counts[b] as f64);
                    (q_a + u_a).partial_cmp(&(q_b + u_b)).unwrap()
                })
                .unwrap();

            // Virtual loss: pretend this node was visited with a neutral score
            visit_counts[best_idx] += 1;
            total_values[best_idx] += VIRTUAL_LOSS_VALUE;

            let pos = legal[best_idx];
            let mut test_plateau = plateau.clone();
            test_plateau.tiles[pos] = *tile;
            let test_deck = replace_tile_in_deck(deck, tile);

            batch_actions.push(best_idx);
            batch_plateaus.push(test_plateau);
            batch_decks.push(test_deck);
        }

        // Run batched rollouts
        let scores = batched_rollouts(
            &batch_plateaus,
            &batch_decks,
            turn + 1,
            policy_net,
            config,
            rng,
        );

        // Backpropagate: undo virtual loss and add real values
        for (i, &action_idx) in batch_actions.iter().enumerate() {
            total_values[action_idx] -= VIRTUAL_LOSS_VALUE; // undo neutral virtual value
            total_values[action_idx] += scores[i] as f64;   // add real score
        }

        sims_done += batch_size;
    }

    // Pick action with highest average value
    let best_idx = (0..n)
        .max_by(|&a, &b| {
            let avg_a = if visit_counts[a] > 0 {
                total_values[a] / visit_counts[a] as f64
            } else {
                0.0
            };
            let avg_b = if visit_counts[b] > 0 {
                total_values[b] / visit_counts[b] as f64
            } else {
                0.0
            };
            avg_a.partial_cmp(&avg_b).unwrap()
        })
        .unwrap();

    legal[best_idx]
}

/// Run N rollouts in parallel with batched GPU inference.
///
/// Each rollout starts from its own (plateau, deck) state and plays to completion.
/// At each turn, all N feature tensors are stacked into `[N, 19, 47]` for a single
/// forward pass, then each rollout picks argmax(logit + line_boost) independently.
fn batched_rollouts(
    plateaus: &[Plateau],
    decks: &[Deck],
    start_turn: usize,
    policy_net: &GraphTransformerPolicyNet,
    config: &BatchedMctsConfig,
    rng: &mut StdRng,
) -> Vec<i32> {
    let n = plateaus.len();
    let mut plateaus: Vec<Plateau> = plateaus.to_vec();
    let mut decks: Vec<Deck> = decks.to_vec();
    let mut active: Vec<bool> = vec![true; n];

    for turn in start_turn..19 {
        // Draw random tiles for each active rollout (CPU)
        let mut tiles: Vec<Tile> = Vec::with_capacity(n);
        for i in 0..n {
            if !active[i] {
                tiles.push(Tile(0, 0, 0));
                continue;
            }
            let available = get_available_tiles(&decks[i]);
            if available.is_empty() {
                active[i] = false;
                tiles.push(Tile(0, 0, 0));
                continue;
            }
            tiles.push(*available.choose(rng).unwrap());
        }

        // Count active rollouts
        let active_indices: Vec<usize> = (0..n).filter(|&i| active[i]).collect();
        if active_indices.is_empty() {
            break;
        }

        // Build features for active rollouts (CPU)
        let features: Vec<Tensor> = active_indices
            .iter()
            .map(|&i| {
                convert_plateau_for_gat_47ch(&plateaus[i], &tiles[i], &decks[i], turn, 19)
            })
            .collect();

        // Stack → [active_count, 19, 47], move to device, single forward pass
        let feat_batch = Tensor::stack(&features, 0).to_device(config.device);
        let logits_batch = tch::no_grad(|| policy_net.forward(&feat_batch, false))
            .to_device(Device::Cpu); // [active_count, 19]

        // For each active rollout: argmax(logit + line_boost) over legal moves
        for (batch_idx, &i) in active_indices.iter().enumerate() {
            let logits: Vec<f64> = Vec::<f64>::try_from(
                logits_batch.get(batch_idx as i64),
            )
            .unwrap();

            let legal = get_legal_moves(&plateaus[i]);
            if legal.is_empty() {
                active[i] = false;
                continue;
            }

            // Mask occupied positions
            let mut mask = [0.0f64; 19];
            for pos in 0..19 {
                if plateaus[i].tiles[pos] != Tile(0, 0, 0) {
                    mask[pos] = f64::NEG_INFINITY;
                }
            }
            let effective_logit = |pos: usize| -> f64 {
                logits[pos] + mask[pos] + line_boost(&plateaus[i], &tiles[i], pos, config.boost)
            };

            let best_pos = *legal
                .iter()
                .max_by(|&&a, &&b| {
                    effective_logit(a).partial_cmp(&effective_logit(b)).unwrap()
                })
                .unwrap();

            plateaus[i].tiles[best_pos] = tiles[i];
            decks[i] = replace_tile_in_deck(&decks[i], &tiles[i]);
        }
    }

    // Score all completed games
    plateaus.iter().map(|p| result(p)).collect()
}

/// Compute softmax prior over legal moves using GT+Boost logits.
fn compute_prior(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    turn: usize,
    policy_net: &GraphTransformerPolicyNet,
    legal: &[usize],
    config: &BatchedMctsConfig,
) -> Vec<f64> {
    let feat = convert_plateau_for_gat_47ch(plateau, tile, deck, turn, 19)
        .unsqueeze(0)
        .to_device(config.device);
    let logits = tch::no_grad(|| policy_net.forward(&feat, false))
        .squeeze_dim(0)
        .to_device(Device::Cpu);
    let logit_values: Vec<f64> = Vec::<f64>::try_from(&logits).unwrap();

    let boosted: Vec<f64> = legal
        .iter()
        .map(|&pos| logit_values[pos] + line_boost(plateau, tile, pos, config.boost))
        .collect();

    // Softmax
    let max_logit = boosted.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let exps: Vec<f64> = boosted.iter().map(|&l| (l - max_logit).exp()).collect();
    let sum: f64 = exps.iter().sum();
    exps.iter().map(|&e| e / sum).collect()
}

/// GT+Boost argmax on the configured device (for last-turn selection).
fn gt_boosted_select_on_device(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    turn: usize,
    policy_net: &GraphTransformerPolicyNet,
    config: &BatchedMctsConfig,
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
