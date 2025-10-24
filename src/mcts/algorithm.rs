//! Core Monte Carlo Tree Search selection loop used by the training pipelines.
//!
//! The algorithm combines neural network priors, handcrafted heuristics and rollout simulations
//! to choose the best placement for a tile. The resulting [`MCTSResult`] snapshot is used both
//! for online play and for generating supervised training data.
use crate::game::deck::Deck;
use crate::game::get_legal_moves::get_legal_moves;
use crate::game::plateau::Plateau;
use crate::game::plateau_is_full::is_plateau_full;
use crate::game::remove_tile_from_deck::replace_tile_in_deck;
use crate::game::simulate_game::simulate_games;
use crate::game::tile::Tile;
use crate::mcts::mcts_result::MCTSResult;
use crate::neural::gnn::convert_plateau_for_gnn;
use crate::neural::manager::NNArchitecture;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::neural::tensor_conversion::convert_plateau_to_tensor;
use crate::scoring::scoring::result;
use crate::strategy::contextual_boost::calculate_contextual_boost_entropy;
use crate::strategy::position_evaluation::enhanced_position_evaluation;
use crate::utils::random_index::random_index;
use std::collections::HashMap;
use tch::{IndexOp, Kind, Tensor};

/// Evaluator used by the MCTS algorithm to rank candidate moves.
pub enum MctsEvaluator<'a> {
    Neural {
        policy_net: &'a PolicyNet,
        value_net: &'a ValueNet,
    },
    #[allow(dead_code)]
    Pure,
}

/// Convenience wrapper retaining the legacy API with neural guidance.
#[allow(clippy::too_many_arguments)]
pub fn mcts_find_best_position_for_tile_with_nn(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    num_simulations: usize,
    current_turn: usize,
    total_turns: usize,
) -> MCTSResult {
    mcts_core(
        plateau,
        deck,
        chosen_tile,
        MctsEvaluator::Neural {
            policy_net,
            value_net,
        },
        num_simulations,
        current_turn,
        total_turns,
    )
}

/// Run MCTS without neural priors/value predictions (pure Monte Carlo rollouts).
#[allow(dead_code)]
pub fn mcts_find_best_position_for_tile_pure(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    num_simulations: usize,
    current_turn: usize,
    total_turns: usize,
) -> MCTSResult {
    mcts_core(
        plateau,
        deck,
        chosen_tile,
        MctsEvaluator::Pure,
        num_simulations,
        current_turn,
        total_turns,
    )
}

#[allow(clippy::too_many_arguments)]
fn mcts_core(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    evaluator: MctsEvaluator<'_>,
    num_simulations: usize,
    current_turn: usize,
    total_turns: usize,
) -> MCTSResult {
    let legal_moves = get_legal_moves(plateau.clone());
    if legal_moves.is_empty() {
        let distribution_len = plateau.tiles.len() as i64;
        let policy_distribution =
            Tensor::zeros([distribution_len], (Kind::Float, tch::Device::Cpu));
        let policy_distribution_boosted = policy_distribution.shallow_clone();
        return MCTSResult {
            best_position: 0,
            board_tensor: convert_plateau_to_tensor(
                plateau,
                &chosen_tile,
                deck,
                current_turn,
                total_turns,
            ),
            subscore: 0.0,
            policy_distribution,
            policy_distribution_boosted,
            boost_intensity: 0.0,
            graph_features: None,
            plateau: Some(plateau.clone()),
            current_turn: Some(current_turn),
            total_turns: Some(total_turns),
        };
    }

    let (input_tensor, graph_features) = match evaluator {
        MctsEvaluator::Neural { policy_net, .. } => {
            let PolicyNet { arch, .. } = policy_net;
            match arch {
                NNArchitecture::CNN => (
                    convert_plateau_to_tensor(
                        plateau,
                        &chosen_tile,
                        deck,
                        current_turn,
                        total_turns,
                    ),
                    None,
                ),
                NNArchitecture::GNN => {
                    let gnn_feat = convert_plateau_for_gnn(plateau, current_turn, total_turns);
                    (gnn_feat.shallow_clone(), Some(gnn_feat))
                }
            }
        }
        _ => (
            convert_plateau_to_tensor(plateau, &chosen_tile, deck, current_turn, total_turns),
            None,
        ),
    };

    let legal_moves = get_legal_moves(plateau.clone());
    if legal_moves.is_empty() {
        let distribution_len = plateau.tiles.len() as i64;
        let policy_distribution =
            Tensor::zeros([distribution_len], (Kind::Float, tch::Device::Cpu));
        let policy_distribution_boosted = policy_distribution.shallow_clone();
        return MCTSResult {
            best_position: 0,
            board_tensor: convert_plateau_to_tensor(
                plateau,
                &chosen_tile,
                deck,
                current_turn,
                total_turns,
            ),
            subscore: 0.0,
            policy_distribution,
            policy_distribution_boosted,
            boost_intensity: 0.0,
            graph_features: None,
            plateau: Some(plateau.clone()),
            current_turn: Some(current_turn),
            total_turns: Some(total_turns),
        };
    }
    let mut value_estimates: HashMap<usize, f64> = HashMap::new();
    let mut min_value = f64::INFINITY;
    let mut max_value = f64::NEG_INFINITY;
    let mut sum_values = 0.0;

    let (policy, entropy_factor) = match evaluator {
        MctsEvaluator::Neural {
            policy_net,
            value_net,
        } => {
            let policy_logits = policy_net.forward(&input_tensor, false);
            let policy = policy_logits.log_softmax(-1, tch::Kind::Float).exp();

            for &position in &legal_moves {
                let mut temp_plateau = plateau.clone();
                let mut temp_deck = deck.clone();

                temp_plateau.tiles[position] = chosen_tile;
                temp_deck = replace_tile_in_deck(&temp_deck, &chosen_tile);

                // Créer le tenseur selon l'architecture (CNN ou GNN)
                let board_tensor_temp = match policy_net.arch {
                    NNArchitecture::CNN => convert_plateau_to_tensor(
                        &temp_plateau,
                        &chosen_tile,
                        &temp_deck,
                        current_turn,
                        total_turns,
                    ),
                    NNArchitecture::GNN => {
                        convert_plateau_for_gnn(&temp_plateau, current_turn, total_turns)
                    }
                };

                let pred_value = value_net
                    .forward(&board_tensor_temp, false)
                    .double_value(&[])
                    .clamp(-1.0, 1.0);

                min_value = min_value.min(pred_value);
                max_value = max_value.max(pred_value);
                sum_values += pred_value;

                value_estimates.insert(position, pred_value);
            }

            let policy_entropy = {
                let policy_float = policy.clamp_min(1e-6);
                let entropy_tensor =
                    -(policy_float.shallow_clone() * policy_float.log()).sum(tch::Kind::Float);
                entropy_tensor.double_value(&[])
            };
            let action_count = policy.size()[1] as f64;
            let max_entropy = if action_count > 0.0 {
                action_count.ln()
            } else {
                1.0
            };
            let normalized_entropy = if max_entropy > 0.0 {
                (policy_entropy / max_entropy).clamp(0.0, 1.0)
            } else {
                1.0
            };

            (policy, normalized_entropy)
        }
        MctsEvaluator::Pure => {
            let num_positions = plateau.tiles.len();
            let mut distribution = vec![0f32; num_positions];

            for &position in &legal_moves {
                distribution[position] = 1.0 / (legal_moves.len() as f32);

                let mut temp_plateau = plateau.clone();
                let mut temp_deck = deck.clone();

                temp_plateau.tiles[position] = chosen_tile;
                temp_deck = replace_tile_in_deck(&temp_deck, &chosen_tile);

                let rollout_count = 6;
                let mut total_simulated_score = 0.0;
                for _ in 0..rollout_count {
                    total_simulated_score +=
                        simulate_games(temp_plateau.clone(), temp_deck.clone()) as f64;
                }
                let avg_score = total_simulated_score / rollout_count as f64;
                let normalized_value = ((avg_score / 200.0).clamp(0.0, 1.0) * 2.0) - 1.0;

                min_value = min_value.min(normalized_value);
                max_value = max_value.max(normalized_value);
                sum_values += normalized_value;

                value_estimates.insert(position, normalized_value);
            }

            (
                Tensor::from_slice(&distribution).view([1, num_positions as i64]),
                1.0,
            )
        }
    };

    let mut visit_counts: HashMap<usize, usize> = HashMap::new();
    let mut total_scores: HashMap<usize, f64> = HashMap::new();
    let mut ucb_scores: HashMap<usize, f64> = HashMap::new();
    let mut ucb_scores_raw: HashMap<usize, f64> = HashMap::new();
    let mut total_visits: i32 = 0;
    for &position in &legal_moves {
        visit_counts.insert(position, 0);
        total_scores.insert(position, 0.0);
        ucb_scores.insert(position, f64::NEG_INFINITY);
    }

    // 🎯 **Dynamic c_puct based on ValueNet variance**
    let mean_value = if value_estimates.is_empty() {
        0.0
    } else {
        sum_values / value_estimates.len() as f64
    };
    let variance = value_estimates
        .values()
        .map(|&v| (v - mean_value).powi(2))
        .sum::<f64>()
        / value_estimates.len() as f64;

    // Adapt c_puct: high variance = more exploration needed
    let base_c_puct = if current_turn < 5 {
        4.2 // Early game base
    } else if current_turn > 15 {
        3.0 // Late game base
    } else {
        3.8 // Mid game base
    };

    // Variance adjustment: 0.0-0.5 variance -> 0.8x-1.3x multiplier
    let variance_multiplier = if variance > 0.5 {
        1.3 // High uncertainty -> explore more
    } else if variance > 0.2 {
        1.1 // Medium uncertainty
    } else if variance > 0.05 {
        1.0 // Low uncertainty
    } else {
        0.85 // Very low uncertainty -> exploit more
    };

    let c_puct = base_c_puct * variance_multiplier;

    if log::log_enabled!(log::Level::Trace) {
        log::trace!(
            "[DynamicMCTS] turn={} variance={:.3} c_puct={:.2} (base={:.2} mult={:.2})",
            current_turn,
            variance,
            c_puct,
            base_c_puct,
            variance_multiplier
        );
    }

    // 🎯 **Improved Dynamic Pruning Strategy**
    // More conservative in early game (keep more options), more aggressive in late game
    let pruning_ratio = if current_turn < 5 {
        0.05 // Keep 95% of moves in very early game (explore broadly)
    } else if current_turn < 10 {
        0.10 // Keep 90% in early-mid game
    } else if current_turn < 15 {
        0.15 // Keep 85% in mid game
    } else {
        0.20 // Keep 80% in late game (focus on best moves)
    };

    let value_threshold = min_value + (max_value - min_value) * pruning_ratio;

    if log::log_enabled!(log::Level::Trace) {
        let kept_moves = legal_moves
            .iter()
            .filter(|&&pos| value_estimates[&pos] >= value_threshold)
            .count();
        log::trace!(
            "[DynamicPruning] turn={} threshold={:.3} keeping {}/{} moves ({}%)",
            current_turn,
            value_threshold,
            kept_moves,
            legal_moves.len(),
            (kept_moves as f64 / legal_moves.len() as f64 * 100.0) as i32
        );
    }

    // Track cumulative boost applied per move for logging/analysis
    let mut boost_applied: HashMap<usize, f64> = HashMap::new();

    for _ in 0..num_simulations {
        let mut moves_with_prior: Vec<_> = legal_moves
            .iter()
            .filter(|&&pos| value_estimates[&pos] >= value_threshold) // Prune weak moves
            .map(|&pos| (pos, policy.i((0, pos as i64)).double_value(&[])))
            .collect();

        moves_with_prior.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let top_k = usize::min(
            moves_with_prior.len(),
            ((total_visits as f64).sqrt() as usize).max(5),
        );

        let subset_moves: Vec<usize> = moves_with_prior
            .iter()
            .take(top_k)
            .map(|&(pos, _)| pos)
            .collect();

        for &position in &subset_moves {
            let mut temp_plateau = plateau.clone();
            let mut temp_deck = deck.clone();

            temp_plateau.tiles[position] = chosen_tile;
            temp_deck = replace_tile_in_deck(&temp_deck, &chosen_tile);

            let value_estimate = *value_estimates.get(&position).unwrap_or(&0.0);

            // **Improved Adaptive Rollout Strategy**
            let rollout_count = match value_estimate {
                x if x > 0.7 => 3,  // Very strong move -> minimal rollouts
                x if x > 0.2 => 5,  // Strong move -> fewer rollouts
                x if x < -0.4 => 9, // Weak/conflicting -> explore more
                _ => 7,             // Default exploration
            };

            let mut total_simulated_score = 0.0;

            for _ in 0..rollout_count {
                let lookahead_plateau = temp_plateau.clone();
                let lookahead_deck = temp_deck.clone();

                // 🔮 Étape 1.1 — Tirer une tuile hypothétique (T2)
                if lookahead_deck.tiles.is_empty() {
                    continue;
                }
                let tile2_index = random_index(lookahead_deck.tiles.len());
                let tile2 = lookahead_deck.tiles[tile2_index];

                // 🔍 Étape 1.2 — Simuler tous les placements possibles de cette tuile
                let second_moves = get_legal_moves(lookahead_plateau.clone());

                let mut best_score_for_tile2: f64 = 0.0;

                for &pos2 in &second_moves {
                    let mut plateau2 = lookahead_plateau.clone();
                    let mut deck2 = lookahead_deck.clone();

                    plateau2.tiles[pos2] = tile2;
                    deck2 = replace_tile_in_deck(&deck2, &tile2);

                    let score = simulate_games(plateau2.clone(), deck2.clone()) as f64;
                    best_score_for_tile2 = best_score_for_tile2.max(score);
                }

                total_simulated_score += best_score_for_tile2;
            }

            let simulated_score = total_simulated_score / rollout_count as f64;

            let visits = visit_counts.entry(position).or_insert(0);
            *visits += 1;
            total_visits += 1;

            let total_score = total_scores.entry(position).or_insert(0.0);
            *total_score += simulated_score;

            let exploration_param = c_puct * (total_visits as f64).ln() / (1.0 + *visits as f64);
            let prior_prob = policy.i((0, position as i64)).double_value(&[]);
            let average_score = *total_score / (*visits as f64);
            let enhanced_eval =
                enhanced_position_evaluation(&temp_plateau, position, &chosen_tile, current_turn);

            let normalized_rollout = ((average_score / 200.0).clamp(0.0, 1.0) * 2.0) - 1.0;
            let normalized_value = value_estimate.clamp(-1.0, 1.0);
            let normalized_heuristic = (enhanced_eval / 30.0).clamp(-1.0, 1.0);
            let contextual = calculate_contextual_boost_entropy(
                plateau,
                position,
                &chosen_tile,
                current_turn,
                entropy_factor,
            )
            .clamp(-1.0, 1.0);

            let combined_eval = 0.6 * normalized_value
                + 0.2 * normalized_rollout
                + 0.1 * normalized_heuristic
                + 0.1 * contextual;

            let ucb_score = combined_eval + exploration_param * prior_prob.max(1e-6).sqrt();

            ucb_scores_raw.insert(position, combined_eval);
            *boost_applied.entry(position).or_insert(0.0) += contextual;

            ucb_scores.insert(position, ucb_score);
        }
    }

    // Select the move with the highest UCB score
    let best_position = legal_moves
        .into_iter()
        .max_by(|&a, &b| {
            ucb_scores
                .get(&a)
                .unwrap_or(&f64::NEG_INFINITY)
                .partial_cmp(ucb_scores.get(&b).unwrap_or(&f64::NEG_INFINITY))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(0);

    // **NEW: Simulate the Rest of the Game to Get Final Score**
    let mut final_plateau = plateau.clone();
    let mut final_deck = deck.clone();
    final_plateau.tiles[best_position] = chosen_tile;
    final_deck = replace_tile_in_deck(&final_deck, &chosen_tile);

    while !is_plateau_full(&final_plateau) {
        let tile_index = random_index(final_deck.tiles.len());
        let random_tile = final_deck.tiles[tile_index];

        let available_moves = get_legal_moves(final_plateau.clone());
        if available_moves.is_empty() {
            break;
        }

        let random_position = available_moves[random_index(available_moves.len())];
        final_plateau.tiles[random_position] = random_tile;
        final_deck = replace_tile_in_deck(&final_deck, &random_tile);
    }

    let final_score = result(&final_plateau); // Get actual game score

    let mut visit_distribution_boosted = vec![0f32; plateau.tiles.len()];
    for (&position, &count) in visit_counts.iter() {
        if position < visit_distribution_boosted.len() {
            visit_distribution_boosted[position] = count as f32;
        }
    }
    let total_boosted_sum: f32 = visit_distribution_boosted.iter().sum();
    if total_boosted_sum > 0.0 {
        for value in &mut visit_distribution_boosted {
            *value /= total_boosted_sum;
        }
    } else if best_position < visit_distribution_boosted.len() {
        visit_distribution_boosted[best_position] = 1.0;
    }

    let mut visit_distribution_raw = vec![0f32; plateau.tiles.len()];
    if !ucb_scores_raw.is_empty() {
        let max_score = ucb_scores_raw
            .values()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        let mut exp_scores: HashMap<usize, f64> = HashMap::new();
        let mut exp_sum = 0.0;
        for (&position, &score) in &ucb_scores_raw {
            let exp_val = (score - max_score).exp();
            exp_sum += exp_val;
            exp_scores.insert(position, exp_val);
        }
        if exp_sum > 0.0 {
            for (&position, &exp_val) in &exp_scores {
                if position < visit_distribution_raw.len() {
                    visit_distribution_raw[position] = (exp_val / exp_sum) as f32;
                }
            }
        }
    }
    let raw_sum: f32 = visit_distribution_raw.iter().sum();
    if raw_sum <= f32::EPSILON && best_position < visit_distribution_raw.len() {
        visit_distribution_raw[best_position] = 1.0;
    }

    let policy_distribution = Tensor::from_slice(&visit_distribution_raw);
    let policy_distribution_boosted = Tensor::from_slice(&visit_distribution_boosted);

    let total_boost: f64 = boost_applied.values().sum();

    MCTSResult {
        best_position,
        board_tensor: input_tensor,
        subscore: final_score as f64, // Store real final score, not UCB score
        policy_distribution,
        policy_distribution_boosted,
        boost_intensity: total_boost as f32,
        graph_features,
        plateau: Some(plateau.clone()),
        current_turn: Some(current_turn),
        total_turns: Some(total_turns),
    }
}
