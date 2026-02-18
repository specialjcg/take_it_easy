//! GT + Line Boost strategy module.
//!
//! Provides line-completion heuristics that augment Graph Transformer logits:
//!   - `line_boost`: logit bonus for line completion/near-completion
//!   - `gt_boosted_select`: GT logits + line_boost → argmax (zero overhead)
//!   - `gt_rollout_boosted`: rollout using GT+Boost as policy
//!   - `gt_beam_rollout_select`: top-K candidates + M boosted rollouts → best avg
//!   - `gt_beam_v1_select`: beam + always inject v1-ideal position as candidate

use rand::prelude::*;
use rand::rngs::StdRng;
use tch::Tensor;

use crate::game::deck::Deck;
use crate::game::get_legal_moves::get_legal_moves;
use crate::game::plateau::Plateau;
use crate::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use crate::game::tile::Tile;
use crate::neural::graph_transformer::GraphTransformerPolicyNet;
use crate::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use crate::scoring::scoring::result;

/// The 15 scoring lines of a Take It Easy board.
///
/// Each entry is (positions, direction_index) where:
///   - direction 0 = tile.0 (horizontal/v1)
///   - direction 1 = tile.1 (diagonal v2)
///   - direction 2 = tile.2 (diagonal v3)
pub const LINES: [(&[usize], usize); 15] = [
    // Horizontal rows (v1)
    (&[0, 1, 2], 0),
    (&[3, 4, 5, 6], 0),
    (&[7, 8, 9, 10, 11], 0),
    (&[12, 13, 14, 15], 0),
    (&[16, 17, 18], 0),
    // Diagonal v2
    (&[0, 3, 7], 1),
    (&[1, 4, 8, 12], 1),
    (&[2, 5, 9, 13, 16], 1),
    (&[6, 10, 14, 17], 1),
    (&[11, 15, 18], 1),
    // Diagonal v3
    (&[7, 12, 16], 2),
    (&[3, 8, 13, 17], 2),
    (&[0, 4, 9, 14, 18], 2),
    (&[1, 5, 10, 15], 2),
    (&[2, 6, 11], 2),
];

/// Compute a logit boost for placing a tile at a given position.
///
/// Only boosts for near-completion and completion — doesn't interfere with GT
/// for early/mid-game decisions.
///
/// Returns a bonus to ADD to the raw GT logit for this position.
///   - Completing a line (last empty slot, all match): +bonus × value × length / 45
///   - Near-completion (penultimate slot, all match): +bonus × 0.4 × ...
///   - Two away, already building: +bonus × 0.15 × ...
///   - Breaking a near-complete homogeneous line: -penalty
pub fn line_boost(plateau: &Plateau, tile: &Tile, position: usize, boost: f64) -> f64 {
    let mut total = 0.0;

    for &(line_positions, direction) in &LINES {
        if !line_positions.contains(&position) {
            continue;
        }

        let tile_val = match direction {
            0 => tile.0,
            1 => tile.1,
            _ => tile.2,
        };

        let line_len = line_positions.len();

        // Count: matching, conflicting, empty (excluding our position)
        let mut same = 0;
        let mut diff = 0;
        let mut empty = 0;
        let mut diff_val = 0;
        let mut diff_homogeneous = true;

        for &pos in line_positions {
            if pos == position {
                continue;
            }
            let t = &plateau.tiles[pos];
            if *t == Tile(0, 0, 0) {
                empty += 1;
            } else {
                let v = match direction {
                    0 => t.0,
                    1 => t.1,
                    _ => t.2,
                };
                if v == tile_val {
                    same += 1;
                } else {
                    if diff_val == 0 {
                        diff_val = v;
                    } else if v != diff_val {
                        diff_homogeneous = false;
                    }
                    diff += 1;
                }
            }
        }

        // Normalize: tile_val in [1,9], line_len in [3,5], max product = 45
        let norm = 45.0;

        if diff == 0 {
            if empty == 0 {
                // COMPLETING the line! Strong boost.
                total += boost * tile_val as f64 * line_len as f64 / norm;
            } else if empty == 1 && same >= 1 {
                // Near-completion: penultimate slot filled, 1 empty remains
                total += boost * 0.4 * tile_val as f64 * line_len as f64 / norm;
            } else if empty == 2 && same >= 1 {
                // Two away from completion, already building
                total += boost * 0.15 * tile_val as f64 * line_len as f64 / norm;
            }
        } else if diff_homogeneous && same == 0 && diff_val != 0 {
            // We're BREAKING a homogeneous partial line
            let progress = diff as f64 / (line_len - 1) as f64;
            total -= boost * 0.3 * progress * diff_val as f64 * line_len as f64 / norm;
        }
    }

    total
}

/// Compute masked GT logits for a board state.
fn gt_masked_logits(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    turn: usize,
    policy_net: &GraphTransformerPolicyNet,
) -> Tensor {
    let feat = convert_plateau_for_gat_47ch(plateau, tile, deck, turn, 19).unsqueeze(0);
    let logits = policy_net.forward(&feat, false).squeeze_dim(0);
    let mut mask = [0.0f32; 19];
    for i in 0..19 {
        if plateau.tiles[i] != Tile(0, 0, 0) {
            mask[i] = f32::NEG_INFINITY;
        }
    }
    logits + Tensor::from_slice(&mask)
}

/// GT logits + line_boost → argmax. Zero overhead over GT Direct.
///
/// Returns the best position index from the legal moves.
pub fn gt_boosted_select(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    turn: usize,
    policy_net: &GraphTransformerPolicyNet,
    boost: f64,
) -> usize {
    let legal = get_legal_moves(plateau);
    if legal.len() <= 1 {
        return legal.first().copied().unwrap_or(0);
    }

    let masked = gt_masked_logits(plateau, tile, deck, turn, policy_net);
    let logit_values: Vec<f64> = Vec::<f64>::try_from(&masked).unwrap();

    *legal
        .iter()
        .max_by(|&&a, &&b| {
            let sa = logit_values[a] + line_boost(plateau, tile, a, boost);
            let sb = logit_values[b] + line_boost(plateau, tile, b, boost);
            sa.partial_cmp(&sb).unwrap()
        })
        .unwrap()
}

/// Rollout a game from a partial state using GT + line_boost as policy.
///
/// Tiles are drawn randomly from the remaining deck. Returns the final score.
pub fn gt_rollout_boosted(
    plateau: &Plateau,
    deck: &Deck,
    current_turn: usize,
    policy_net: &GraphTransformerPolicyNet,
    boost: f64,
    rng: &mut StdRng,
) -> i32 {
    let mut plateau = plateau.clone();
    let mut deck = deck.clone();

    for turn in current_turn..19 {
        let available = get_available_tiles(&deck);
        if available.is_empty() {
            break;
        }
        let tile = *available.choose(rng).unwrap();

        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        let masked = gt_masked_logits(&plateau, &tile, &deck, turn, policy_net);
        let logit_values: Vec<f64> = Vec::<f64>::try_from(&masked).unwrap();

        let best_pos = *legal
            .iter()
            .max_by(|&&a, &&b| {
                let sa = logit_values[a] + line_boost(&plateau, &tile, a, boost);
                let sb = logit_values[b] + line_boost(&plateau, &tile, b, boost);
                sa.partial_cmp(&sb).unwrap()
            })
            .unwrap();

        plateau.tiles[best_pos] = tile;
        deck = replace_tile_in_deck(&deck, &tile);
    }

    result(&plateau)
}

/// Rollout using GT + line_boost + v1_row_priority as policy.
///
/// Like `gt_rollout_boosted`, but also applies a bonus when placing a tile
/// in its v1-ideal row (v1=9→center, v1=5→sides, v1=1→edges), so the
/// rollout policy "understands" the v1-row strategy.
pub fn gt_rollout_v1_aware(
    plateau: &Plateau,
    deck: &Deck,
    current_turn: usize,
    policy_net: &GraphTransformerPolicyNet,
    boost: f64,
    v1_bonus: f64,
    rng: &mut StdRng,
) -> i32 {
    let mut plateau = plateau.clone();
    let mut deck = deck.clone();

    for turn in current_turn..19 {
        let available = get_available_tiles(&deck);
        if available.is_empty() {
            break;
        }
        let tile = *available.choose(rng).unwrap();

        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        let masked = gt_masked_logits(&plateau, &tile, &deck, turn, policy_net);
        let logit_values: Vec<f64> = Vec::<f64>::try_from(&masked).unwrap();

        let best_pos = *legal
            .iter()
            .max_by(|&&a, &&b| {
                let sa = logit_values[a]
                    + line_boost(&plateau, &tile, a, boost)
                    + v1_row_bonus(&plateau, &tile, a, v1_bonus);
                let sb = logit_values[b]
                    + line_boost(&plateau, &tile, b, boost)
                    + v1_row_bonus(&plateau, &tile, b, v1_bonus);
                sa.partial_cmp(&sb).unwrap()
            })
            .unwrap();

        plateau.tiles[best_pos] = tile;
        deck = replace_tile_in_deck(&deck, &tile);
    }

    result(&plateau)
}

/// V1-row bonus: reward placing a tile in its ideal v1 row if viable.
///
/// Returns +bonus if tile.v1 matches the target v1 for this row and the row
/// is still viable (no conflicting v1 already placed).
/// Returns -bonus*0.3 for strong mismatches (v1=9 on edge, v1=1 on center).
/// Returns 0.0 otherwise.
fn v1_row_bonus(plateau: &Plateau, tile: &Tile, position: usize, bonus: f64) -> f64 {
    let row_idx = match position {
        0..=2 => 0,
        3..=6 => 1,
        7..=11 => 2,
        12..=15 => 3,
        16..=18 => 4,
        _ => return 0.0,
    };
    let target_v1 = ROW_TARGET_V1[row_idx];

    // Check row viability
    let viable = ROWS[row_idx].iter().all(|&pos| {
        let t = &plateau.tiles[pos];
        *t == Tile(0, 0, 0) || t.0 == tile.0
    });

    if tile.0 == target_v1 && viable {
        bonus
    } else if (tile.0 == 9 && row_idx == 0 || tile.0 == 9 && row_idx == 4)
        || (tile.0 == 1 && row_idx == 2)
    {
        // Strong mismatch: 9 on edges or 1 on center
        -bonus * 0.3
    } else {
        0.0
    }
}

/// PUCT MCTS with GT+Boost as prior and rollout policy.
///
/// 1. Prior: softmax(GT_logits + line_boost) for legal positions
/// 2. PUCT selection loop (num_sims iterations)
/// 3. Rollout: gt_rollout_boosted from state after placement
/// 4. Final: argmax(W(a) / N(a)) — highest average value
pub fn gt_mcts_select(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    turn: usize,
    policy_net: &GraphTransformerPolicyNet,
    boost: f64,
    num_sims: usize,
    rng: &mut StdRng,
) -> usize {
    let legal = get_legal_moves(plateau);
    if legal.len() <= 1 {
        return legal.first().copied().unwrap_or(0);
    }
    if turn >= 18 {
        return gt_boosted_select(plateau, tile, deck, turn, policy_net, boost);
    }

    const C_PUCT: f64 = 2.5;
    const SCORE_NORM: f64 = 300.0;

    // Compute GT+Boost prior: softmax(gt_logits + line_boost)
    let masked = gt_masked_logits(plateau, tile, deck, turn, policy_net);
    let logit_values: Vec<f64> = Vec::<f64>::try_from(&masked).unwrap();

    let boosted_logits: Vec<f64> = legal
        .iter()
        .map(|&pos| logit_values[pos] + line_boost(plateau, tile, pos, boost))
        .collect();

    // Softmax over legal positions
    let max_logit = boosted_logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let exp_sum: f64 = boosted_logits.iter().map(|&l| (l - max_logit).exp()).collect::<Vec<f64>>().iter().sum();
    let priors: Vec<f64> = boosted_logits.iter().map(|&l| (l - max_logit).exp() / exp_sum).collect();

    let n = legal.len();
    let mut visit_counts = vec![0usize; n];
    let mut total_values = vec![0.0f64; n];

    for _ in 0..num_sims {
        let total_visits: usize = visit_counts.iter().sum();
        let sqrt_total = (total_visits as f64).sqrt();

        // PUCT selection
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
                let u_a = C_PUCT * priors[a] * sqrt_total / (1.0 + visit_counts[a] as f64);
                let u_b = C_PUCT * priors[b] * sqrt_total / (1.0 + visit_counts[b] as f64);
                (q_a + u_a).partial_cmp(&(q_b + u_b)).unwrap()
            })
            .unwrap();

        let pos = legal[best_idx];
        let mut test_plateau = plateau.clone();
        test_plateau.tiles[pos] = *tile;
        let test_deck = replace_tile_in_deck(deck, tile);

        let score = gt_rollout_boosted(&test_plateau, &test_deck, turn + 1, policy_net, boost, rng);

        visit_counts[best_idx] += 1;
        total_values[best_idx] += score as f64;
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

/// Top-K candidates (GT+Boost) evaluated with M boosted rollouts → best avg score.
///
/// 1. Compute GT logits + line_boost for all legal positions
/// 2. Take top `beam_k` candidates
/// 3. For each candidate, run `num_rollouts` rollouts using `gt_rollout_boosted`
/// 4. Return position with highest average rollout score
pub fn gt_beam_rollout_select(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    turn: usize,
    policy_net: &GraphTransformerPolicyNet,
    boost: f64,
    beam_k: usize,
    num_rollouts: usize,
    rng: &mut StdRng,
) -> usize {
    let legal = get_legal_moves(plateau);
    if legal.len() <= 1 {
        return legal.first().copied().unwrap_or(0);
    }

    // Last turn: no rollouts needed
    if turn >= 18 {
        return gt_boosted_select(plateau, tile, deck, turn, policy_net, boost);
    }

    // GT logits + line_boost → ranked candidates
    let masked = gt_masked_logits(plateau, tile, deck, turn, policy_net);
    let logit_values: Vec<f64> = Vec::<f64>::try_from(&masked).unwrap();

    let mut ranked: Vec<(usize, f64)> = legal
        .iter()
        .map(|&pos| (pos, logit_values[pos] + line_boost(plateau, tile, pos, boost)))
        .collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    ranked.truncate(beam_k);

    let candidates: Vec<usize> = ranked.iter().map(|&(pos, _)| pos).collect();

    // Evaluate each candidate with boosted rollouts
    let mut best_pos = candidates[0];
    let mut best_avg = f64::NEG_INFINITY;

    for &pos in &candidates {
        let mut test_plateau = plateau.clone();
        test_plateau.tiles[pos] = *tile;
        let test_deck = replace_tile_in_deck(deck, tile);

        let mut total: i64 = 0;
        for _ in 0..num_rollouts {
            total +=
                gt_rollout_boosted(&test_plateau, &test_deck, turn + 1, policy_net, boost, rng)
                    as i64;
        }
        let avg = total as f64 / num_rollouts as f64;

        if avg > best_avg {
            best_avg = avg;
            best_pos = pos;
        }
    }

    best_pos
}

// ─── V1-row ideal position helpers ─────────────────────────────

/// Horizontal rows: positions belonging to each row.
const ROWS: [&[usize]; 5] = [
    &[0, 1, 2],
    &[3, 4, 5, 6],
    &[7, 8, 9, 10, 11],
    &[12, 13, 14, 15],
    &[16, 17, 18],
];

/// Target v1 for each row: row0=1, row1=5, row2=9, row3=5, row4=1.
const ROW_TARGET_V1: [i32; 5] = [1, 5, 9, 5, 1];

/// Find the best legal position in a v1-ideal row for this tile.
///
/// Returns the legal position in the tile's target row that has the highest
/// GT+Boost logit, or None if no legal position exists in the target rows.
fn best_v1_ideal_pos(
    plateau: &Plateau,
    tile: &Tile,
    legal: &[usize],
    logit_values: &[f64],
    boost: f64,
) -> Option<usize> {
    // Find which rows are targets for this tile's v1
    let target_rows: Vec<usize> = ROW_TARGET_V1
        .iter()
        .enumerate()
        .filter(|(_, &v)| v == tile.0)
        .map(|(i, _)| i)
        .collect();

    // Find legal positions in target rows, check row is still viable
    let mut candidates: Vec<usize> = Vec::new();
    for &row_idx in &target_rows {
        // Check row viability: no conflicting v1 placed
        let viable = ROWS[row_idx].iter().all(|&pos| {
            let t = &plateau.tiles[pos];
            *t == Tile(0, 0, 0) || t.0 == tile.0
        });
        if !viable {
            continue;
        }
        for &pos in ROWS[row_idx] {
            if legal.contains(&pos) {
                candidates.push(pos);
            }
        }
    }

    if candidates.is_empty() {
        return None;
    }

    // Pick the candidate with highest GT+Boost logit
    candidates
        .iter()
        .max_by(|&&a, &&b| {
            let sa = logit_values[a] + line_boost(plateau, tile, a, boost);
            let sb = logit_values[b] + line_boost(plateau, tile, b, boost);
            sa.partial_cmp(&sb).unwrap()
        })
        .copied()
}

/// Top-K GT+Boost candidates + v1-ideal position, evaluated with rollouts.
///
/// Like `gt_beam_rollout_select`, but always injects the best v1-ideal position
/// as a candidate. This ensures the AI evaluates the "human-like" v1-row
/// strategy alongside its own diagonal-optimized approach.
///
/// When the v1-row placement leads to a higher expected score (favorable draw),
/// the rollouts will surface it. When it doesn't, GT's candidate wins naturally.
pub fn gt_beam_v1_select(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    turn: usize,
    policy_net: &GraphTransformerPolicyNet,
    boost: f64,
    beam_k: usize,
    num_rollouts: usize,
    v1_bonus: f64,
    rng: &mut StdRng,
) -> usize {
    let legal = get_legal_moves(plateau);
    if legal.len() <= 1 {
        return legal.first().copied().unwrap_or(0);
    }

    if turn >= 18 {
        return gt_boosted_select(plateau, tile, deck, turn, policy_net, boost);
    }

    // GT logits + line_boost → ranked candidates
    let masked = gt_masked_logits(plateau, tile, deck, turn, policy_net);
    let logit_values: Vec<f64> = Vec::<f64>::try_from(&masked).unwrap();

    let mut ranked: Vec<(usize, f64)> = legal
        .iter()
        .map(|&pos| (pos, logit_values[pos] + line_boost(plateau, tile, pos, boost)))
        .collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    ranked.truncate(beam_k);

    let mut candidates: Vec<usize> = ranked.iter().map(|&(pos, _)| pos).collect();

    // Inject v1-ideal position if not already a candidate
    if let Some(v1_pos) = best_v1_ideal_pos(plateau, tile, &legal, &logit_values, boost) {
        if !candidates.contains(&v1_pos) {
            candidates.push(v1_pos);
        }
    }

    // Evaluate each candidate with v1-aware rollouts
    let mut best_pos = candidates[0];
    let mut best_avg = f64::NEG_INFINITY;

    for &pos in &candidates {
        let mut test_plateau = plateau.clone();
        test_plateau.tiles[pos] = *tile;
        let test_deck = replace_tile_in_deck(deck, tile);

        let mut total: i64 = 0;
        for _ in 0..num_rollouts {
            total += gt_rollout_v1_aware(
                &test_plateau, &test_deck, turn + 1, policy_net, boost, v1_bonus, rng,
            ) as i64;
        }
        let avg = total as f64 / num_rollouts as f64;

        if avg > best_avg {
            best_avg = avg;
            best_pos = pos;
        }
    }

    best_pos
}
