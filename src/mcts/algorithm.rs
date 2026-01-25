//! Core Monte Carlo Tree Search selection loop used by the training pipelines.
//!
//! The algorithm combines neural network priors, handcrafted heuristics and rollout simulations
//! to choose the best placement for a tile. The resulting [`MCTSResult`] snapshot is used both
//! for online play and for generating supervised training data.
use crate::game::deck::Deck;
use crate::game::deck_cow::DeckCoW;
use crate::game::get_legal_moves::get_legal_moves;
use crate::game::plateau::Plateau;
use crate::game::plateau_cow::PlateauCoW;
use crate::game::plateau_is_full::is_plateau_full;
use crate::game::remove_tile_from_deck::{replace_tile_in_deck, replace_tile_in_deck_cow};
use crate::game::simulate_game_smart::{simulate_games_smart, simulate_games_smart_with_trace};
use crate::game::tile::Tile;
use crate::mcts::hyperparameters::MCTSHyperparameters;
use crate::mcts::mcts_result::MCTSResult;
use crate::mcts::progressive_widening::{max_actions_to_explore, ProgressiveWideningConfig};
use crate::neural::gnn::convert_plateau_for_gnn;
use crate::neural::manager::NNArchitecture;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::neural::qvalue_net::QValueNet;
use crate::neural::tensor_conversion::convert_plateau_to_tensor;
use crate::scoring::scoring::result;
use crate::strategy::contextual_boost::calculate_contextual_boost_entropy;
use crate::strategy::position_evaluation::enhanced_position_evaluation;
use crate::utils::random_index::random_index;
use std::collections::HashMap;
use tch::{IndexOp, Kind, Tensor};

/// Helper function to convert plateau to tensor based on architecture
fn convert_plateau_by_arch(
    arch: NNArchitecture,
    plateau: &Plateau,
    chosen_tile: &Tile,
    deck: &Deck,
    current_turn: usize,
    total_turns: usize,
) -> Tensor {
    match arch {
        NNArchitecture::Cnn => convert_plateau_to_tensor(plateau, chosen_tile, deck, current_turn, total_turns),
        NNArchitecture::Gnn => {
            // GNN uses same encoding as CNN (includes tile), then reshaped to [batch, 19, 8]
            convert_plateau_to_tensor(plateau, chosen_tile, deck, current_turn, total_turns)
        },
        NNArchitecture::CnnOnehot => {
            // Use one-hot oriented encoding (37 channels)
            crate::neural::tensor_onehot::convert_plateau_onehot(plateau, chosen_tile, deck, current_turn)
        },
    }
}

/// Evaluator used by the MCTS algorithm to rank candidate moves.
pub enum MctsEvaluator<'a> {
    Neural {
        policy_net: &'a PolicyNet,
        value_net: &'a ValueNet,
    },
    /// Neural + Q-net for action pruning (hybrid mode)
    NeuralWithQNet {
        policy_net: &'a PolicyNet,
        value_net: &'a ValueNet,
        qvalue_net: &'a QValueNet,
        prune_top_k: usize,
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
    hyperparams: Option<&MCTSHyperparameters>,
) -> MCTSResult {
    let default_hyperparams = MCTSHyperparameters::default();
    let hyperparams = hyperparams.unwrap_or(&default_hyperparams);

    // Zero-Copy optimization: Wrap in CoW to eliminate 36K+ clones
    let plateau_cow = PlateauCoW::new(plateau.clone());
    let deck_cow = DeckCoW::new(deck.clone());

    mcts_core_cow(
        &plateau_cow,
        &deck_cow,
        chosen_tile,
        MctsEvaluator::Neural {
            policy_net,
            value_net,
        },
        num_simulations,
        current_turn,
        total_turns,
        hyperparams,
    )
}

/// Run MCTS with neural networks AND Q-net pruning (hybrid mode).
/// Q-net prunes low-quality positions, then CNN policy/value guide MCTS on remaining.
/// Uses adaptive pruning: more aggressive early game, less late game.
#[allow(clippy::too_many_arguments)]
pub fn mcts_find_best_position_for_tile_with_qnet(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    qvalue_net: &QValueNet,
    num_simulations: usize,
    current_turn: usize,
    total_turns: usize,
    prune_top_k: usize,
    hyperparams: Option<&MCTSHyperparameters>,
) -> MCTSResult {
    let default_hyperparams = MCTSHyperparameters::default();
    let hyperparams = hyperparams.unwrap_or(&default_hyperparams);

    // Count empty positions
    let empty_count = plateau.tiles.iter().filter(|t| **t == Tile(0, 0, 0)).count();

    // Adaptive pruning: only prune if enough positions and early game
    // Fine-tuned: turn_threshold=10 is optimal (prune turns 0-9 only)
    let should_prune = empty_count > prune_top_k + 2 && current_turn < 10;

    if should_prune {
        // Q-net pruning - get top-K positions by ranking
        let top_positions = qvalue_net.get_top_positions(&plateau.tiles, &chosen_tile, prune_top_k);

        // Run simpler MCTS with rollouts only on top positions
        // (CNN doesn't work well with masked positions)
        let mut best_pos = top_positions[0];
        let mut best_score = f64::NEG_INFINITY;

        let sims_per_pos = num_simulations / top_positions.len().max(1);

        for &pos in &top_positions {
            let mut temp_plateau = plateau.clone();
            temp_plateau.tiles[pos] = chosen_tile;
            let temp_deck = replace_tile_in_deck(deck, &chosen_tile);

            let mut total = 0.0;
            for _ in 0..sims_per_pos {
                total += simulate_games_smart(temp_plateau.clone(), temp_deck.clone(), None) as f64;
            }
            let avg = total / sims_per_pos as f64;

            if avg > best_score {
                best_score = avg;
                best_pos = pos;
            }
        }

        // Create result with best position from Q-net filtered rollouts
        let board_tensor = convert_plateau_by_arch(
            policy_net.arch,
            plateau,
            &chosen_tile,
            deck,
            current_turn,
            total_turns,
        );

        // Create uniform distribution for filtered positions
        let mut distribution = vec![0.0f32; 19];
        for &pos in &top_positions {
            distribution[pos] = 1.0 / top_positions.len() as f32;
        }

        MCTSResult {
            best_position: best_pos,
            board_tensor,
            subscore: best_score,
            policy_distribution: Tensor::from_slice(&distribution).view([19]),
            policy_distribution_boosted: Tensor::from_slice(&distribution).view([19]),
            boost_intensity: 0.0,
            graph_features: None,
            plateau: Some(plateau.clone()),
            current_turn: Some(current_turn),
            total_turns: Some(total_turns),
            q_value_distribution: None,
        }
    } else {
        // Late game or few positions: use full CNN MCTS without pruning
        mcts_find_best_position_for_tile_with_nn(
            plateau,
            deck,
            chosen_tile,
            policy_net,
            value_net,
            num_simulations,
            current_turn,
            total_turns,
            Some(hyperparams),
        )
    }
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
    hyperparams: Option<&MCTSHyperparameters>,
) -> MCTSResult {
    let default_hyperparams = MCTSHyperparameters::default();
    let hyperparams = hyperparams.unwrap_or(&default_hyperparams);

    // Zero-Copy optimization: Wrap in CoW to eliminate clone overhead
    let plateau_cow = PlateauCoW::new(plateau.clone());
    let deck_cow = DeckCoW::new(deck.clone());

    mcts_core_cow(
        &plateau_cow,
        &deck_cow,
        chosen_tile,
        MctsEvaluator::Pure,
        num_simulations,
        current_turn,
        total_turns,
        hyperparams,
    )
}

/// Run MCTS with Gumbel selection instead of UCB
/// This variant uses Gumbel-Top-k sampling for better exploration
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub fn mcts_find_best_position_for_tile_gumbel(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    num_simulations: usize,
    current_turn: usize,
    total_turns: usize,
    _hyperparams: Option<&MCTSHyperparameters>,
) -> MCTSResult {
    // Note: Gumbel variant doesn't use hyperparams yet
    mcts_core_gumbel(
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

#[allow(clippy::too_many_arguments)]
fn mcts_core(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    evaluator: MctsEvaluator<'_>,
    num_simulations: usize,
    current_turn: usize,
    total_turns: usize,
    hyperparams: &MCTSHyperparameters,
) -> MCTSResult {
    // Extract architecture from evaluator
    let arch = match &evaluator {
        MctsEvaluator::Neural { policy_net, .. } => policy_net.arch,
        MctsEvaluator::NeuralWithQNet { policy_net, .. } => policy_net.arch,
        MctsEvaluator::Pure => NNArchitecture::Cnn, // default
    };

    let legal_moves = get_legal_moves(plateau);
    if legal_moves.is_empty() {
        let distribution_len = plateau.tiles.len() as i64;
        let policy_distribution =
            Tensor::zeros([distribution_len], (Kind::Float, tch::Device::Cpu));
        let policy_distribution_boosted = policy_distribution.shallow_clone();
        return MCTSResult {
            best_position: 0,
            board_tensor: convert_plateau_by_arch(
                arch,
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
            q_value_distribution: None,
        };
    }

    let (input_tensor, graph_features) = match &evaluator {
        MctsEvaluator::Neural { policy_net, .. } | MctsEvaluator::NeuralWithQNet { policy_net, .. } => {
            let PolicyNet { arch, .. } = policy_net;
            match arch {
                NNArchitecture::Cnn => (
                    convert_plateau_to_tensor(
                        plateau,
                        &chosen_tile,
                        deck,
                        current_turn,
                        total_turns,
                    ),
                    None,
                ),
                NNArchitecture::CnnOnehot => (
                    crate::neural::tensor_onehot::convert_plateau_onehot(
                        plateau,
                        &chosen_tile,
                        deck,
                        current_turn,
                    ),
                    None,
                ),
                NNArchitecture::Gnn => {
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

    let legal_moves = get_legal_moves(plateau);
    if legal_moves.is_empty() {
        let distribution_len = plateau.tiles.len() as i64;
        let policy_distribution =
            Tensor::zeros([distribution_len], (Kind::Float, tch::Device::Cpu));
        let policy_distribution_boosted = policy_distribution.shallow_clone();
        return MCTSResult {
            best_position: 0,
            board_tensor: convert_plateau_by_arch(
                arch,
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
            q_value_distribution: None,
        };
    }
    let mut value_estimates: HashMap<usize, f64> = HashMap::new();
    let mut min_value = f64::INFINITY;
    let mut max_value = f64::NEG_INFINITY;
    let mut sum_values = 0.0;

    // ADAPTIVE ENTROPY WEIGHTING: Calculate policy entropy for dynamic weight adjustment
    let adaptive_weights = match &evaluator {
        MctsEvaluator::Neural { policy_net, .. } | MctsEvaluator::NeuralWithQNet { policy_net, .. } => {
            let policy_logits = policy_net.forward(&input_tensor, false);
            let policy = policy_logits.log_softmax(-1, tch::Kind::Float).exp();

            // Extract policy probabilities for entropy calculation
            let policy_probs: Vec<f32> = policy.view([-1]).try_into().unwrap_or_else(|_| vec![]);

            if !policy_probs.is_empty() {
                // HYBRID STRATEGY: Turn-adaptive + Entropy-based
                let (w_cnn, w_rollout) = hyperparams.get_hybrid_adaptive_weights(current_turn, &policy_probs);

                // Log adaptive weights at key turns (0, 5, 10, 15) to track strategy
                let debug_turns = [0, 5, 10, 15];
                if debug_turns.contains(&current_turn) {
                    // Calculate entropy for logging
                    let mut entropy = 0.0;
                    for &p in &policy_probs {
                        if p > 1e-10 {
                            entropy -= (p as f64) * (p as f64).ln();
                        }
                    }
                    let max_entropy = (policy_probs.len() as f64).ln();
                    let normalized_entropy = (entropy / max_entropy).clamp(0.0, 1.0);

                    // Compare strategies
                    let (turn_w_cnn, turn_w_rollout) = hyperparams.get_turn_adaptive_weights(current_turn);
                    let (entropy_w_cnn, entropy_w_rollout) = hyperparams.get_adaptive_cnn_weight(&policy_probs);

                    log::info!("🎯 ADAPTIVE STRATEGY COMPARISON (turn {}):", current_turn);
                    log::info!("   Entropy: {:.3} (normalized: {:.3})", entropy, normalized_entropy);
                    log::info!("   Turn-only:    w_cnn={:.3}, w_rollout={:.3}", turn_w_cnn, turn_w_rollout);
                    log::info!("   Entropy-only: w_cnn={:.3}, w_rollout={:.3}", entropy_w_cnn, entropy_w_rollout);
                    log::info!("   HYBRID:       w_cnn={:.3}, w_rollout={:.3} ← ACTIVE", w_cnn, w_rollout);
                }

                Some((w_cnn, w_rollout))
            } else {
                None
            }
        }
        _ => None,
    };

    let (policy, entropy_factor) = match &evaluator {
        MctsEvaluator::Neural {
            policy_net,
            value_net,
        } | MctsEvaluator::NeuralWithQNet {
            policy_net,
            value_net,
            ..
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
                    NNArchitecture::Cnn => convert_plateau_to_tensor(
                        &temp_plateau,
                        &chosen_tile,
                        &temp_deck,
                        current_turn,
                        total_turns,
                    ),
                    NNArchitecture::CnnOnehot => crate::neural::tensor_onehot::convert_plateau_onehot(
                        &temp_plateau,
                        &chosen_tile,
                        &temp_deck,
                        current_turn,
                    ),
                    NNArchitecture::Gnn => {
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

                let rollout_count = hyperparams.rollout_default;
                let mut total_simulated_score = 0.0;
                for _ in 0..rollout_count {
                    total_simulated_score +=
                        simulate_games_smart(temp_plateau.clone(), temp_deck.clone(), None) as f64;
                    // Note: clone needed here as temp_plateau/temp_deck used multiple times in loop
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

    // RAVE disabled - incompatible with Pattern Rollouts heuristics
    // Pattern Rollouts biases introduce false correlations in RAVE statistics

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
    let base_c_puct = hyperparams.get_c_puct(current_turn);

    // Variance adjustment: 0.0-0.5 variance -> 0.8x-1.3x multiplier
    let variance_multiplier = hyperparams.get_variance_multiplier(variance);

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
    let pruning_ratio = hyperparams.get_pruning_ratio(current_turn);

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

    // Quick Win #1: Adaptive simulations based on game phase
    let adaptive_simulations = hyperparams.get_adaptive_simulations(current_turn, num_simulations);

    // Quick Win #2: Temperature annealing for exploration/exploitation
    let temperature = hyperparams.get_temperature(current_turn);

    // Progressive Widening: Dynamically limit action exploration based on visit count
    // Formula: k(n) = C × n^α where n = total_visits
    // Adapts exploration breadth to confidence level (more visits = wider exploration)
    let pw_config = ProgressiveWideningConfig::adaptive(current_turn, total_turns);
    let max_actions = max_actions_to_explore(
        total_visits as usize,
        legal_moves.len(),
        &pw_config,
    );

    for _ in 0..adaptive_simulations {
        // FIXED: Don't filter/sort by CNN when it's undertrained
        // Use all legal moves with uniform prior instead of CNN-based pruning
        let subset_moves: Vec<usize> = legal_moves.clone();

        for &position in &subset_moves {
            let mut temp_plateau = plateau.clone();
            let mut temp_deck = deck.clone();

            temp_plateau.tiles[position] = chosen_tile;
            temp_deck = replace_tile_in_deck(&temp_deck, &chosen_tile);

            let value_estimate = *value_estimates.get(&position).unwrap_or(&0.0);

            // **Improved Adaptive Rollout Strategy**
            let rollout_count = hyperparams.get_rollout_count(value_estimate);

            let mut total_simulated_score = 0.0;

            for _ in 0..rollout_count {
                // 🔮 Étape 1.1 — Tirer une tuile hypothétique (T2)
                if temp_deck.tiles.is_empty() {
                    continue;
                }
                let tile2_index = random_index(temp_deck.tiles.len());
                let tile2 = temp_deck.tiles[tile2_index];

                // 🔍 Étape 1.2 — Simuler tous les placements possibles de cette tuile
                let second_moves = get_legal_moves(&temp_plateau);

                let mut best_score_for_tile2: f64 = 0.0;

                for &pos2 in &second_moves {
                    let mut plateau2 = temp_plateau.clone();
                    let mut deck2 = temp_deck.clone();

                    plateau2.tiles[pos2] = tile2;
                    deck2 = replace_tile_in_deck(&deck2, &tile2);

                    // Pattern Rollouts V2: Smart heuristic-based simulation (moved ownership)
                    let score = simulate_games_smart(plateau2, deck2, None) as f64;
                    best_score_for_tile2 = best_score_for_tile2.max(score);
                }

                total_simulated_score += best_score_for_tile2;
            }

            let simulated_score = total_simulated_score / rollout_count as f64;

            // Update MCTS statistics
            let visits = visit_counts.entry(position).or_insert(0);
            *visits += 1;
            total_visits += 1;

            let total_score = total_scores.entry(position).or_insert(0.0);
            *total_score += simulated_score;

            // Apply temperature annealing to exploration term (Quick Win #2)
            let exploration_param =
                temperature * c_puct * (total_visits as f64).ln() / (1.0 + *visits as f64);
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

            // Pattern Rollouts V2: Weighted combination of evaluators
            // Use adaptive weights if available (entropy-based), otherwise use fixed hyperparams
            let (w_cnn, w_rollout) = adaptive_weights.unwrap_or((hyperparams.weight_cnn, hyperparams.weight_rollout));
            let combined_eval = w_cnn * normalized_value
                + w_rollout * normalized_rollout
                + hyperparams.weight_heuristic * normalized_heuristic
                + hyperparams.weight_contextual * contextual;

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

        let available_moves = get_legal_moves(&final_plateau);
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

    // STOCHZERO: Compute Q-value based policy distribution
    let q_value_dist = create_q_value_policy_target(&value_estimates, 19, 1.0);

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
        q_value_distribution: Some(q_value_dist),
    }
}

/// Copy-on-Write version of mcts_core - eliminates 36,750+ clone operations
///
/// Uses PlateauCoW and DeckCoW to share immutable data across simulations,
/// only cloning when modifications are needed. Expected performance improvement:
/// - Allocations: -97% (from 36,750 to <1,000 per call)
/// - CPU time: -30% (from profiling analysis)
/// - Score: +20-40 pts (from reduced overhead allowing more simulations)
#[allow(clippy::too_many_arguments)]
fn mcts_core_cow(
    plateau_cow: &PlateauCoW,
    deck_cow: &DeckCoW,
    chosen_tile: Tile,
    evaluator: MctsEvaluator<'_>,
    num_simulations: usize,
    current_turn: usize,
    total_turns: usize,
    hyperparams: &MCTSHyperparameters,
) -> MCTSResult {
    // Extract architecture from evaluator
    let arch = match &evaluator {
        MctsEvaluator::Neural { policy_net, .. } => policy_net.arch,
        MctsEvaluator::NeuralWithQNet { policy_net, .. } => policy_net.arch,
        MctsEvaluator::Pure => NNArchitecture::Cnn, // default
    };

    // Extract owned Plateau/Deck for read-only operations that need them
    let plateau = &plateau_cow.read(|p| p.clone());
    let deck = &deck_cow.read(|d| d.clone());

    let legal_moves = get_legal_moves(plateau);
    if legal_moves.is_empty() {
        let distribution_len = plateau.tiles.len() as i64;
        let policy_distribution =
            Tensor::zeros([distribution_len], (Kind::Float, tch::Device::Cpu));
        let policy_distribution_boosted = policy_distribution.shallow_clone();
        return MCTSResult {
            best_position: 0,
            board_tensor: convert_plateau_by_arch(
                arch,
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
            q_value_distribution: None,
        };
    }

    let (input_tensor, graph_features) = match &evaluator {
        MctsEvaluator::Neural { policy_net, .. } | MctsEvaluator::NeuralWithQNet { policy_net, .. } => {
            let PolicyNet { arch, .. } = policy_net;
            match arch {
                NNArchitecture::Cnn => (
                    convert_plateau_to_tensor(
                        plateau,
                        &chosen_tile,
                        deck,
                        current_turn,
                        total_turns,
                    ),
                    None,
                ),
                NNArchitecture::CnnOnehot => (
                    crate::neural::tensor_onehot::convert_plateau_onehot(
                        plateau,
                        &chosen_tile,
                        deck,
                        current_turn,
                    ),
                    None,
                ),
                NNArchitecture::Gnn => {
                    let gnn_feat = convert_plateau_for_gnn(plateau, current_turn, total_turns);
                    (gnn_feat.shallow_clone(), Some(gnn_feat))
                }
            }
        }
        MctsEvaluator::Pure | MctsEvaluator::NeuralWithQNet { .. } => (
            convert_plateau_to_tensor(
                plateau,
                &chosen_tile,
                deck,
                current_turn,
                total_turns,
            ),
            None,
        ),
    };

    let mut value_estimates: HashMap<usize, f64> = HashMap::new();
    let mut min_value = f64::MAX;
    let mut max_value = f64::MIN;
    let mut sum_values = 0.0;

    // ADAPTIVE ENTROPY WEIGHTING: Calculate policy entropy for dynamic weight adjustment
    let adaptive_weights = match &evaluator {
        MctsEvaluator::Neural { policy_net, .. } | MctsEvaluator::NeuralWithQNet { policy_net, .. } => {
            let policy_logits = policy_net.forward(&input_tensor, false);
            let policy = policy_logits.log_softmax(-1, tch::Kind::Float).exp();

            // Extract policy probabilities for entropy calculation
            let policy_probs: Vec<f32> = policy.view([-1]).try_into().unwrap_or_else(|_| vec![]);

            if !policy_probs.is_empty() {
                // HYBRID STRATEGY: Turn-adaptive + Entropy-based
                let (w_cnn, w_rollout) = hyperparams.get_hybrid_adaptive_weights(current_turn, &policy_probs);

                // Log adaptive weights at key turns (0, 5, 10, 15) to track strategy
                let debug_turns = [0, 5, 10, 15];
                if debug_turns.contains(&current_turn) {
                    // Calculate entropy for logging
                    let mut entropy = 0.0;
                    for &p in &policy_probs {
                        if p > 1e-10 {
                            entropy -= (p as f64) * (p as f64).ln();
                        }
                    }
                    let max_entropy = (policy_probs.len() as f64).ln();
                    let normalized_entropy = (entropy / max_entropy).clamp(0.0, 1.0);

                    // Compare strategies
                    let (turn_w_cnn, turn_w_rollout) = hyperparams.get_turn_adaptive_weights(current_turn);
                    let (entropy_w_cnn, entropy_w_rollout) = hyperparams.get_adaptive_cnn_weight(&policy_probs);

                    log::info!("🎯 ADAPTIVE STRATEGY COMPARISON (turn {}):", current_turn);
                    log::info!("   Entropy: {:.3} (normalized: {:.3})", entropy, normalized_entropy);
                    log::info!("   Turn-only:    w_cnn={:.3}, w_rollout={:.3}", turn_w_cnn, turn_w_rollout);
                    log::info!("   Entropy-only: w_cnn={:.3}, w_rollout={:.3}", entropy_w_cnn, entropy_w_rollout);
                    log::info!("   HYBRID:       w_cnn={:.3}, w_rollout={:.3} ← ACTIVE", w_cnn, w_rollout);
                }

                Some((w_cnn, w_rollout))
            } else {
                None
            }
        }
        _ => None,
    };

    let (policy, _policy_entropy) = match &evaluator {
        MctsEvaluator::Neural {
            policy_net,
            value_net,
        } | MctsEvaluator::NeuralWithQNet {
            policy_net,
            value_net,
            ..
        } => {
            let policy_logits = policy_net.forward(&input_tensor, false);
            let policy = policy_logits.log_softmax(-1, tch::Kind::Float).exp();

            // FIXED: Use rollouts for initial value estimates instead of CNN
            // CNN value predictions were causing catastrophic decisions
            for &position in &legal_moves {
                let temp_plateau_cow = plateau_cow.clone_for_modification();
                temp_plateau_cow.set_tile(position, chosen_tile);
                let temp_deck_cow = replace_tile_in_deck_cow(deck_cow, &chosen_tile);

                // Use rollouts instead of CNN value for initial estimate
                let rollout_count = hyperparams.rollout_default;
                let mut total_simulated_score = 0.0;
                for _ in 0..rollout_count {
                    total_simulated_score += simulate_games_smart(
                        temp_plateau_cow.read(|p| p.clone()),
                        temp_deck_cow.read(|d| d.clone()),
                        None,
                    ) as f64;
                }
                let avg_score = total_simulated_score / rollout_count as f64;
                let value = ((avg_score / 200.0).clamp(0.0, 1.0) * 2.0) - 1.0;

                min_value = min_value.min(value);
                max_value = max_value.max(value);
                sum_values += value;

                value_estimates.insert(position, value);
            }

            let policy_entropy = {
                let policy_probs = policy.shallow_clone().softmax(-1, Kind::Float);
                let log_probs = policy_probs.log();
                let entropy_tensor = -(policy_probs * log_probs).sum(Kind::Float);
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

                let temp_plateau_cow = plateau_cow.clone_for_modification();
                temp_plateau_cow.set_tile(position, chosen_tile);
                let temp_deck_cow = replace_tile_in_deck_cow(deck_cow, &chosen_tile);

                let rollout_count = hyperparams.rollout_default;
                let mut total_simulated_score = 0.0;
                for _ in 0..rollout_count {
                    total_simulated_score += simulate_games_smart(
                        temp_plateau_cow.read(|p| p.clone()),
                        temp_deck_cow.read(|d| d.clone()),
                        None,
                    ) as f64;
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

    // RAVE statistics (Sprint 3: Rapid Action Value Estimation)
    let mut rave_visits: HashMap<usize, usize> = HashMap::new();
    let mut rave_scores: HashMap<usize, f64> = HashMap::new();

    let mut total_visits: i32 = 0;
    for &position in &legal_moves {
        visit_counts.insert(position, 0);
        total_scores.insert(position, 0.0);
        ucb_scores.insert(position, f64::NEG_INFINITY);
        rave_visits.insert(position, 0);
        rave_scores.insert(position, 0.0);
    }

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

    let base_c_puct = hyperparams.get_c_puct(current_turn);
    let variance_multiplier = hyperparams.get_variance_multiplier(variance);
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

    let pruning_ratio = hyperparams.get_pruning_ratio(current_turn);
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

    let mut boost_applied: HashMap<usize, f64> = HashMap::new();
    let adaptive_simulations = hyperparams.get_adaptive_simulations(current_turn, num_simulations);
    let temperature = hyperparams.get_temperature(current_turn);

    let pw_config = ProgressiveWideningConfig::adaptive(current_turn, total_turns);
    let max_actions = max_actions_to_explore(
        total_visits as usize,
        legal_moves.len(),
        &pw_config,
    );

    // DEBUG: Log MCTS configuration on first simulation of first turn
    let debug_first_turn = false; // Disabled for dataset generation performance
    if debug_first_turn {
        log::info!("🔍 DEBUG MCTS turn 0:");
        log::info!("   legal_moves.len()={}, max_actions={}, adaptive_simulations={}",
            legal_moves.len(), max_actions, adaptive_simulations);
        log::info!("   pruning_ratio={:.3}, value_threshold={:.3}, min_value={:.3}, max_value={:.3}",
            pruning_ratio, value_threshold, min_value, max_value);
    }

    for sim_idx in 0..adaptive_simulations {
        // FIXED: Don't filter/sort by CNN when it's undertrained
        // Use all legal moves with uniform prior instead of CNN-based pruning
        let subset_moves: Vec<usize> = legal_moves.clone();

        // DEBUG: Log first simulation
        if debug_first_turn && sim_idx == 0 {
            log::info!("   First simulation: subset_moves={:?}", subset_moves);
        }

        // CRITICAL SECTION: Zero-Copy refactor
        // BEFORE: 8 expensive clones per iteration = 880,800 total operations
        // AFTER: Cheap Rc clones + clone_for_modification() only when mutating
        for &position in &subset_moves {
            // ✅ Cheap clone (Rc increment only, no Vec allocation)
            let temp_plateau_cow = plateau_cow.clone_for_modification();
            let temp_deck_cow = deck_cow.clone_for_modification();

            // Direct mutation via RefCell (no additional clone needed)
            temp_plateau_cow.set_tile(position, chosen_tile);
            let temp_deck_cow = replace_tile_in_deck_cow(&temp_deck_cow, &chosen_tile);

            let value_estimate = *value_estimates.get(&position).unwrap_or(&0.0);
            let rollout_count = hyperparams.get_rollout_count(value_estimate);

            let mut total_simulated_score = 0.0;

            // CRITICAL FIX: Only run rollouts if rollout_count > 0
            // Otherwise simulated_score = 0/0 = NaN, which propagates through combined_eval
            // even when weight_rollout = 0.0 (because 0.0 * NaN = NaN in IEEE 754)
            let simulated_score = if rollout_count == 0 {
                0.0  // No rollouts = assume average value
            } else {
                for _ in 0..rollout_count {
                // ✅ Cheap clone (Rc increment, was expensive Vec clone before)
                let lookahead_plateau_cow = temp_plateau_cow.clone();
                let lookahead_deck_cow = temp_deck_cow.clone();

                // Read-only access
                let deck_tiles_len = lookahead_deck_cow.read(|d| d.tiles.len());
                if deck_tiles_len == 0 {
                    continue;
                }

                let tile2_index = random_index(deck_tiles_len);
                let tile2 = lookahead_deck_cow.read(|d| d.tiles[tile2_index]);

                let second_moves = lookahead_plateau_cow.read(|p| get_legal_moves(p));

                let mut best_score_for_tile2: f64 = 0.0;

                for &pos2 in &second_moves {
                    // ✅ Cheap clone (was expensive Vec clone before)
                    let plateau2_cow = lookahead_plateau_cow.clone_for_modification();
                    let deck2_cow = lookahead_deck_cow.clone_for_modification();

                    plateau2_cow.set_tile(pos2, tile2);
                    let deck2_cow = replace_tile_in_deck_cow(&deck2_cow, &tile2);

                    // RAVE: Use with_trace to get positions played during rollout
                    let (score, positions_played) = simulate_games_smart_with_trace(
                        plateau2_cow.into_inner(),  // ✅ Consumes CoW wrapper
                        deck2_cow.into_inner(),      // ✅ Consumes CoW wrapper
                        None,
                    );
                    let score = score as f64;
                    best_score_for_tile2 = best_score_for_tile2.max(score);

                    // RAVE: Update statistics for all positions in rollout (All-Moves-As-First heuristic)
                    for &played_pos in &positions_played {
                        if legal_moves.contains(&played_pos) {
                            *rave_visits.entry(played_pos).or_insert(0) += 1;
                            *rave_scores.entry(played_pos).or_insert(0.0) += score;
                        }
                    }
                }

                total_simulated_score += best_score_for_tile2;
            }

                total_simulated_score / rollout_count as f64
            };

            let visits = visit_counts.entry(position).or_insert(0);
            *visits += 1;
            total_visits += 1;

            let total_score = total_scores.entry(position).or_insert(0.0);
            *total_score += simulated_score;

            let exploration_param =
                temperature * c_puct * (total_visits as f64).ln() / (1.0 + *visits as f64);
            let prior_prob = policy.i((0, position as i64)).double_value(&[]);
            let average_score = *total_score / (*visits as f64);

            let temp_plateau = temp_plateau_cow.read(|p| p.clone());
            let enhanced_eval =
                enhanced_position_evaluation(&temp_plateau, position, &chosen_tile, current_turn);

            let normalized_rollout = ((average_score / 200.0).clamp(0.0, 1.0) * 2.0) - 1.0;
            let normalized_value = value_estimates[&position];
            let normalized_heuristic = (enhanced_eval / 30.0).clamp(-1.0, 1.0);

            let contextual = calculate_contextual_boost_entropy(
                &temp_plateau,
                position,
                &chosen_tile,
                current_turn,
                0.5,
            );

            // HYBRID ADAPTIVE WEIGHTS: Use turn-based + entropy-based strategy
            // Early game (0-5):   20% GNN,  70% rollout (GNN weak, trust rollouts)
            // Mid game (6-11):    45% GNN,  45% rollout (balanced)
            // Late game (12+):    75% GNN,  15% rollout (GNN strong, trust policy)
            let (w_cnn, w_rollout) = adaptive_weights.unwrap_or((hyperparams.weight_cnn, hyperparams.weight_rollout));
            let combined_eval = w_cnn * normalized_value
                + w_rollout * normalized_rollout
                + hyperparams.weight_heuristic * normalized_heuristic
                + hyperparams.weight_contextual * contextual;

            // RAVE: DISABLED for diagnostics - causes variance issues (0-158 pts range)
            let final_eval = combined_eval; // Force no RAVE contribution

            let ucb_score = final_eval + exploration_param * prior_prob.max(1e-6).sqrt();

            ucb_scores_raw.insert(position, combined_eval);
            *boost_applied.entry(position).or_insert(0.0) += contextual;

            ucb_scores.insert(position, ucb_score);
        }
    }

    // DEBUG: Log UCB scores before selection
    if debug_first_turn {
        let mut ucb_vec: Vec<(usize, f64)> = ucb_scores.iter().map(|(&pos, &score)| (pos, score)).collect();
        ucb_vec.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        log::info!("   UCB scores (top 5): {:?}",
            ucb_vec.iter().take(5).map(|(pos, score)| format!("pos{}:{:.4}", pos, score)).collect::<Vec<_>>());
        log::info!("   Total visits: {}", total_visits);
        let visits_vec: Vec<(usize, usize)> = visit_counts.iter().map(|(&pos, &count)| (pos, count)).collect();
        log::info!("   Visit counts (first 5): {:?}",
            visits_vec.iter().take(5).map(|(pos, count)| format!("pos{}:{}", pos, count)).collect::<Vec<_>>());
    }

    // For supervised learning: use value_estimates (initial rollouts Q-values) for exploitation
    // This gives deterministic expert moves based on rollout quality, not UCB exploration
    let best_position = legal_moves
        .into_iter()
        .max_by(|&a, &b| {
            value_estimates
                .get(&a)
                .unwrap_or(&f64::NEG_INFINITY)
                .partial_cmp(value_estimates.get(&b).unwrap_or(&f64::NEG_INFINITY))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(0);

    // DEBUG: Log selected position
    if debug_first_turn {
        log::info!("   Selected best_position: {} with UCB score: {:.4}",
            best_position, ucb_scores.get(&best_position).unwrap_or(&f64::NEG_INFINITY));
    }

    // Final simulation using owned values (acceptable, happens once per move)
    let mut final_plateau = plateau.clone();
    let mut final_deck = deck.clone();
    final_plateau.tiles[best_position] = chosen_tile;
    final_deck = replace_tile_in_deck(&final_deck, &chosen_tile);

    while !is_plateau_full(&final_plateau) {
        let tile_index = random_index(final_deck.tiles.len());
        let random_tile = final_deck.tiles[tile_index];

        let available_moves = get_legal_moves(&final_plateau);
        if available_moves.is_empty() {
            break;
        }

        let random_position = available_moves[random_index(available_moves.len())];
        final_plateau.tiles[random_position] = random_tile;
        final_deck = replace_tile_in_deck(&final_deck, &random_tile);
    }

    let final_score = result(&final_plateau);

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

    // STOCHZERO: Compute Q-value based policy distribution
    // This provides stronger learning signal than visit counts (which are uniform with 200 sims)
    let q_value_dist = create_q_value_policy_target(&value_estimates, 19, 1.0);

    MCTSResult {
        best_position,
        board_tensor: input_tensor,
        subscore: final_score as f64,
        policy_distribution,
        policy_distribution_boosted,
        boost_intensity: total_boost as f32,
        graph_features,
        plateau: Some(plateau.clone()),
        current_turn: Some(current_turn),
        total_turns: Some(total_turns),
        q_value_distribution: Some(q_value_dist),
    }
}

/// Create policy distribution from Q-values using softmax with temperature.
/// This allows the policy network to learn from rollout quality rather than visit counts.
///
/// # Arguments
/// * `q_values` - HashMap mapping positions to their Q-values (average rollout scores)
/// * `plateau_size` - Number of positions on the plateau
/// * `temperature` - Controls how sharply to focus on high Q-values (0.5 = focused, 1.0 = smooth)
///
/// # Returns
/// Tensor of shape [plateau_size] with softmax probabilities based on Q-values
fn create_q_value_policy_target(
    q_values: &HashMap<usize, f64>,
    plateau_size: usize,
    temperature: f64,
) -> Tensor {
    let mut policy = vec![0.0f32; plateau_size];

    if q_values.is_empty() {
        // Fallback: uniform distribution
        let uniform_prob = 1.0 / plateau_size as f32;
        policy.fill(uniform_prob);
        return Tensor::from_slice(&policy);
    }

    // Extract Q-values for all positions
    let positions: Vec<usize> = q_values.keys().copied().collect();
    let q_vec: Vec<f64> = positions.iter().map(|&pos| *q_values.get(&pos).unwrap_or(&0.0)).collect();

    if q_vec.is_empty() {
        return Tensor::from_slice(&policy);
    }

    // Softmax with temperature: exp((Q - max_Q) / temperature) / sum(exp(...))
    let max_q = q_vec.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let exp_q: Vec<f64> = q_vec
        .iter()
        .map(|&q| ((q - max_q) / temperature).exp())
        .collect();
    let sum_exp: f64 = exp_q.iter().sum();

    // Avoid division by zero
    if sum_exp < f64::EPSILON {
        let uniform_prob = 1.0 / positions.len() as f32;
        for &pos in &positions {
            if pos < plateau_size {
                policy[pos] = uniform_prob;
            }
        }
    } else {
        // Assign probabilities based on softmax
        for (i, &pos) in positions.iter().enumerate() {
            if pos < plateau_size {
                policy[pos] = (exp_q[i] / sum_exp) as f32;
            }
        }
    }

    Tensor::from_slice(&policy)
}

/// Gumbel MCTS Core - Uses Gumbel noise instead of UCB for selection
#[allow(clippy::too_many_arguments)]
fn mcts_core_gumbel(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    evaluator: MctsEvaluator<'_>,
    num_simulations: usize,
    current_turn: usize,
    total_turns: usize,
) -> MCTSResult {
    use crate::mcts::gumbel_selection::{gumbel_select, GumbelSelector};

    // Extract architecture from evaluator
    let arch = match &evaluator {
        MctsEvaluator::Neural { policy_net, .. } => policy_net.arch,
        MctsEvaluator::NeuralWithQNet { policy_net, .. } => policy_net.arch,
        MctsEvaluator::Pure => NNArchitecture::Cnn, // default
    };

    let legal_moves = get_legal_moves(plateau);
    if legal_moves.is_empty() {
        let distribution_len = plateau.tiles.len() as i64;
        let policy_distribution =
            Tensor::zeros([distribution_len], (Kind::Float, tch::Device::Cpu));
        let policy_distribution_boosted = policy_distribution.shallow_clone();
        return MCTSResult {
            best_position: 0,
            board_tensor: convert_plateau_by_arch(
                arch,
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
            q_value_distribution: None,
        };
    }

    let (input_tensor, graph_features) = match &evaluator {
        MctsEvaluator::Neural { policy_net, .. } | MctsEvaluator::NeuralWithQNet { policy_net, .. } => {
            let PolicyNet { arch, .. } = policy_net;
            match arch {
                NNArchitecture::Cnn => (
                    convert_plateau_to_tensor(
                        plateau,
                        &chosen_tile,
                        deck,
                        current_turn,
                        total_turns,
                    ),
                    None,
                ),
                NNArchitecture::CnnOnehot => (
                    crate::neural::tensor_onehot::convert_plateau_onehot(
                        plateau,
                        &chosen_tile,
                        deck,
                        current_turn,
                    ),
                    None,
                ),
                NNArchitecture::Gnn => {
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

    let mut value_estimates: HashMap<usize, f64> = HashMap::new();
    let mut min_value = f64::INFINITY;
    let mut max_value = f64::NEG_INFINITY;

    let (_policy, _entropy_factor) = match &evaluator {
        MctsEvaluator::Neural {
            policy_net,
            value_net,
        } | MctsEvaluator::NeuralWithQNet {
            policy_net,
            value_net,
            ..
        } => {
            let policy_logits = policy_net.forward(&input_tensor, false);
            let policy = policy_logits.log_softmax(-1, tch::Kind::Float).exp();

            for &position in &legal_moves {
                let mut temp_plateau = plateau.clone();
                let mut temp_deck = deck.clone();

                temp_plateau.tiles[position] = chosen_tile;
                temp_deck = replace_tile_in_deck(&temp_deck, &chosen_tile);

                let board_tensor_temp = match policy_net.arch {
                    NNArchitecture::Cnn => convert_plateau_to_tensor(
                        &temp_plateau,
                        &chosen_tile,
                        &temp_deck,
                        current_turn,
                        total_turns,
                    ),
                    NNArchitecture::CnnOnehot => crate::neural::tensor_onehot::convert_plateau_onehot(
                        &temp_plateau,
                        &chosen_tile,
                        &temp_deck,
                        current_turn,
                    ),
                    NNArchitecture::Gnn => {
                        convert_plateau_for_gnn(&temp_plateau, current_turn, total_turns)
                    }
                };

                let pred_value = value_net
                    .forward(&board_tensor_temp, false)
                    .double_value(&[])
                    .clamp(-1.0, 1.0);

                min_value = min_value.min(pred_value);
                max_value = max_value.max(pred_value);

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

                let rollout_count = 6; // Gumbel uses hardcoded values (not part of hyperparams system)
                let mut total_simulated_score = 0.0;

                for _ in 0..rollout_count {
                    total_simulated_score +=
                        simulate_games_smart(temp_plateau.clone(), temp_deck.clone(), None) as f64;
                    // Note: clone needed here as temp_plateau/temp_deck used multiple times in loop
                }
                let avg_score = total_simulated_score / rollout_count as f64;
                let normalized_value = ((avg_score / 200.0).clamp(0.0, 1.0) * 2.0) - 1.0;

                min_value = min_value.min(normalized_value);
                max_value = max_value.max(normalized_value);

                value_estimates.insert(position, normalized_value);
            }

            (
                Tensor::from_slice(&distribution).view([1, num_positions as i64]),
                1.0,
            )
        }
    };

    let mut visit_counts: HashMap<usize, usize> = HashMap::new();
    let mut q_values: HashMap<usize, f64> = HashMap::new();

    for &position in &legal_moves {
        visit_counts.insert(position, 0);
        q_values.insert(position, 0.0);
    }

    // Adaptive temperature for Gumbel selection
    let temperature = GumbelSelector::adaptive_temperature(current_turn, total_turns);

    if log::log_enabled!(log::Level::Trace) {
        log::trace!(
            "[GumbelMCTS] turn={} temperature={:.2}",
            current_turn,
            temperature
        );
    }

    // Run simulations
    for sim_idx in 0..num_simulations {
        // Use Gumbel selection for move selection
        let top_k = 5; // Consider top 5 candidates
        let selected_position = if sim_idx < legal_moves.len() {
            // First N simulations: ensure each move visited at least once
            legal_moves[sim_idx]
        } else {
            // Use Gumbel selection
            match gumbel_select(&q_values, &visit_counts, temperature, top_k) {
                Some(pos) => pos,
                None => legal_moves[random_index(legal_moves.len())],
            }
        };

        let mut temp_plateau = plateau.clone();
        let mut temp_deck = deck.clone();

        temp_plateau.tiles[selected_position] = chosen_tile;
        temp_deck = replace_tile_in_deck(&temp_deck, &chosen_tile);

        let value_estimate = *value_estimates.get(&selected_position).unwrap_or(&0.0);

        // Adaptive rollout count
        let rollout_count = match value_estimate {
            x if x > 0.7 => 3,
            x if x > 0.2 => 5,
            x if x < -0.4 => 9,
            _ => 7,
        };

        let mut total_simulated_score = 0.0;

        for _ in 0..rollout_count {
            if temp_deck.tiles.is_empty() {
                continue;
            }
            let tile2_index = random_index(temp_deck.tiles.len());
            let tile2 = temp_deck.tiles[tile2_index];

            let second_moves = get_legal_moves(&temp_plateau);
            let mut best_score_for_tile2: f64 = 0.0;

            for &pos2 in &second_moves {
                let mut plateau2 = temp_plateau.clone();
                let mut deck2 = temp_deck.clone();

                plateau2.tiles[pos2] = tile2;
                deck2 = replace_tile_in_deck(&deck2, &tile2);

                let score = simulate_games_smart(plateau2, deck2, None) as f64;
                best_score_for_tile2 = best_score_for_tile2.max(score);
            }

            total_simulated_score += best_score_for_tile2;
        }

        let simulated_score = total_simulated_score / rollout_count as f64;

        // Update Q-values using incremental average
        let visits = visit_counts.entry(selected_position).or_insert(0);
        *visits += 1;

        let q_value = q_values.entry(selected_position).or_insert(0.0);
        let normalized_score = ((simulated_score / 200.0).clamp(0.0, 1.0) * 2.0) - 1.0;
        *q_value += (normalized_score - *q_value) / (*visits as f64);
    }

    // Select best move based on Q-values (greedy, exploitation)
    // For supervised learning, we want deterministic expert moves based on rollout quality
    let best_position = legal_moves
        .into_iter()
        .max_by(|&a, &b| {
            q_values
                .get(&a)
                .unwrap_or(&f64::NEG_INFINITY)
                .partial_cmp(q_values.get(&b).unwrap_or(&f64::NEG_INFINITY))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(0);

    // Simulate rest of game for final score
    let mut final_plateau = plateau.clone();
    let mut final_deck = deck.clone();
    final_plateau.tiles[best_position] = chosen_tile;
    final_deck = replace_tile_in_deck(&final_deck, &chosen_tile);

    while !is_plateau_full(&final_plateau) {
        let tile_index = random_index(final_deck.tiles.len());
        let random_tile = final_deck.tiles[tile_index];

        let available_moves = get_legal_moves(&final_plateau);
        if available_moves.is_empty() {
            break;
        }

        let random_position = available_moves[random_index(available_moves.len())];
        final_plateau.tiles[random_position] = random_tile;
        final_deck = replace_tile_in_deck(&final_deck, &random_tile);
    }

    let final_score = result(&final_plateau);

    // Create policy distributions
    let mut visit_distribution = vec![0f32; plateau.tiles.len()];
    for (&position, &count) in visit_counts.iter() {
        if position < visit_distribution.len() {
            visit_distribution[position] = count as f32;
        }
    }
    let total_visits: f32 = visit_distribution.iter().sum();
    if total_visits > 0.0 {
        for value in &mut visit_distribution {
            *value /= total_visits;
        }
    } else if best_position < visit_distribution.len() {
        visit_distribution[best_position] = 1.0;
    }

    let policy_distribution = Tensor::from_slice(&visit_distribution);
    let policy_distribution_boosted = policy_distribution.shallow_clone();

    // Create Q-value based policy target (for training to learn from rollout quality)
    let q_value_distribution = Some(create_q_value_policy_target(
        &q_values,
        plateau.tiles.len(),
        0.5, // Temperature: 0.5 = focused on best moves
    ));

    MCTSResult {
        best_position,
        board_tensor: input_tensor,
        subscore: final_score as f64,
        policy_distribution,
        policy_distribution_boosted,
        boost_intensity: 0.0,
        graph_features,
        plateau: Some(plateau.clone()),
        current_turn: Some(current_turn),
        total_turns: Some(total_turns),
        q_value_distribution,
    }
}

/// UCT-based MCTS that samples ONE position per simulation (not batch exploration)
/// This allows the policy network to influence exploration and breaks uniform data generation
///
/// # Dirichlet Noise Support
/// The `exploration_priors` parameter allows adding Dirichlet noise for exploration
/// during self-play (AlphaGo Zero technique). This breaks circular learning where
/// uniform policy → uniform MCTS → uniform training data → uniform policy.
#[allow(clippy::too_many_arguments)]
pub fn mcts_find_best_position_for_tile_uct(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    num_simulations: usize,
    current_turn: usize,
    total_turns: usize,
    hyperparams: Option<&MCTSHyperparameters>,
    exploration_priors: Option<Vec<f32>>, // Dirichlet noise for self-play exploration
) -> MCTSResult {
    let default_hyperparams = MCTSHyperparameters::default();
    let hyperparams = hyperparams.unwrap_or(&default_hyperparams);

    // Extract architecture from policy_net
    let arch = policy_net.arch;

    let legal_moves = get_legal_moves(plateau);
    if legal_moves.is_empty() {
        let distribution_len = plateau.tiles.len() as i64;
        let policy_distribution =
            Tensor::zeros([distribution_len], (Kind::Float, tch::Device::Cpu));
        return MCTSResult {
            best_position: 0,
            board_tensor: convert_plateau_by_arch(
                arch,
                plateau,
                &chosen_tile,
                deck,
                current_turn,
                total_turns,
            ),
            subscore: 0.0,
            policy_distribution: policy_distribution.shallow_clone(),
            policy_distribution_boosted: policy_distribution,
            boost_intensity: 0.0,
            graph_features: None,
            plateau: Some(plateau.clone()),
            current_turn: Some(current_turn),
            total_turns: Some(total_turns),
            q_value_distribution: None,
        };
    }

    // Get policy and value priors from neural networks
    let input_tensor = convert_plateau_by_arch(
        arch,
        plateau,
        &chosen_tile,
        deck,
        current_turn,
        total_turns,
    );

    let policy_logits = policy_net.forward(&input_tensor, false);
    let policy_probs = policy_logits.softmax(1, Kind::Float);
    let value_prior = value_net.forward(&input_tensor, false).double_value(&[0, 0]);

    // Extract policy probabilities for legal moves
    let mut policy_vec: Vec<f64> = Vec::new();
    for &pos in &legal_moves {
        let prob = policy_probs.i((0, pos as i64)).double_value(&[]);
        policy_vec.push(prob);
    }

    // Normalize policy probabilities (only over legal moves)
    let sum: f64 = policy_vec.iter().sum();
    if sum > 0.0 {
        for prob in &mut policy_vec {
            *prob /= sum;
        }
    } else {
        // Uniform if all zero
        let uniform = 1.0 / legal_moves.len() as f64;
        policy_vec = vec![uniform; legal_moves.len()];
    }

    // ====================================================================
    // MIX DIRICHLET NOISE WITH POLICY (AlphaGo Zero technique)
    // ====================================================================
    // If exploration_priors provided, mix them with network policy:
    // mixed_prior = (1 - ε) * policy_prior + ε * dirichlet_noise
    if let Some(ref noise_vec) = exploration_priors {
        let epsilon = 0.5; // Mix ratio: 50% policy + 50% noise (STRENGTHENED to break circular learning)

        // DEBUG: Log policy evolution across game (turn 0, 5, 10, 15)
        let debug_turns = [0, 5, 10, 15];
        let should_log = debug_turns.contains(&current_turn) && num_simulations == 150;
        if should_log {
            log::info!("🔍 DEBUG MCTS Mixing (turn {}):", current_turn);
            log::info!("   Policy BEFORE mix: {:?}", policy_vec.iter().take(5).map(|v| format!("{:.3}", v)).collect::<Vec<_>>());
        }

        for (idx, &pos) in legal_moves.iter().enumerate() {
            let noise_value = noise_vec.get(pos).copied().unwrap_or(0.0) as f64;
            policy_vec[idx] = (1.0 - epsilon) * policy_vec[idx] + epsilon * noise_value;
        }
        // Re-normalize after mixing
        let sum_after_mix: f64 = policy_vec.iter().sum();
        if sum_after_mix > 0.0 {
            for prob in &mut policy_vec {
                *prob /= sum_after_mix;
            }
        }

        // DEBUG: Log after mixing
        if should_log {
            log::info!("   Policy AFTER mix: {:?}", policy_vec.iter().take(5).map(|v| format!("{:.3}", v)).collect::<Vec<_>>());
        }
    }

    // LOG ADAPTIVE WEIGHTING STRATEGY
    let debug_turns = [0, 5, 10, 15];
    if debug_turns.contains(&current_turn) && num_simulations == 150 {
        let (w_cnn, w_rollout) = hyperparams.get_turn_adaptive_weights(current_turn);
        log::info!("🎯 TURN-ADAPTIVE WEIGHTS (turn {}):", current_turn);
        log::info!("   w_cnn={:.3}, w_rollout={:.3}", w_cnn, w_rollout);
        if current_turn <= 5 {
            log::info!("   Phase: EARLY GAME (rollout-heavy, GNN weak)");
        } else if current_turn <= 11 {
            log::info!("   Phase: MID GAME (balanced)");
        } else {
            log::info!("   Phase: LATE GAME (GNN-dominant)");
        }
    }

    // Initialize UCT statistics
    let mut visit_counts: HashMap<usize, usize> = HashMap::new();
    let mut total_values: HashMap<usize, f64> = HashMap::new();

    for &pos in &legal_moves {
        visit_counts.insert(pos, 0);
        total_values.insert(pos, 0.0);
    }

    let total_simulations = hyperparams.get_adaptive_simulations(current_turn, num_simulations);

    // UCT simulation loop - ONE position per simulation
    for sim_idx in 0..total_simulations {
        // Selection: Choose ONE position using UCT formula + policy prior
        let total_visits = sim_idx + 1;
        let exploration_const = hyperparams.get_c_puct(current_turn);

        let mut best_ucb = f64::NEG_INFINITY;
        let mut best_position = legal_moves[0];

        for (idx, &pos) in legal_moves.iter().enumerate() {
            let visits = visit_counts[&pos];
            let mean_value = if visits > 0 {
                total_values[&pos] / visits as f64
            } else {
                value_prior  // Use value network as prior
            };

            // UCT formula: Q(s,a) + c * P(s,a) * sqrt(N) / (1 + n(a))
            let prior_prob = policy_vec[idx];
            let ucb_score = mean_value
                + exploration_const * prior_prob * (total_visits as f64).sqrt()
                    / (1.0 + visits as f64);

            if ucb_score > best_ucb {
                best_ucb = ucb_score;
                best_position = pos;
            }
        }

        // Simulate this position
        let mut temp_plateau = plateau.clone();
        let mut temp_deck = deck.clone();

        temp_plateau.tiles[best_position] = chosen_tile;
        temp_deck = replace_tile_in_deck(&temp_deck, &chosen_tile);

        // HEX-AWARE HEURISTIC EVALUATION (No rollouts! Fast!)
        // Q = immediate_score + completion_potential + hex_degree_bonus

        // 1. Calculate immediate score from placing tile
        let immediate_score = calculate_score_for_plateau(&temp_plateau);
        let base_score = calculate_score_for_plateau(plateau);
        let score_gain = (immediate_score - base_score) as f64;

        // 2. Completion potential: count lines that are 2/3+ filled with same value
        let completion_potential = calculate_line_completion_potential(&temp_plateau, current_turn, total_turns);

        // 3. Hex degree bonus: center positions touch more lines (weight: 1-6)
        let hex_degree = get_hex_degree(best_position) as f64 / 6.0;  // Normalize to [0,1]

        // Combined Q-value (normalized to [-1, 1] range)
        let q_value = (score_gain * 0.5) + (completion_potential * 0.3) + (hex_degree * 0.2);
        let normalized_value = (q_value / 100.0).clamp(-1.0, 1.0);

        // Update statistics
        *visit_counts.get_mut(&best_position).unwrap() += 1;
        *total_values.get_mut(&best_position).unwrap() += normalized_value;
    }

    // Select best move based on visit counts (most explored)
    let mut best_position = legal_moves[0];
    let mut max_visits = 0;

    for &pos in &legal_moves {
        let visits = visit_counts[&pos];
        if visits > max_visits {
            max_visits = visits;
            best_position = pos;
        }
    }

    // DEBUG: Log visit count distribution (turns 0, 5, 10, 15)
    let debug_turns = [0, 5, 10, 15];
    let should_log_visits = debug_turns.contains(&current_turn) && num_simulations == 150;
    if should_log_visits {
        let mut sorted_visits: Vec<(usize, usize)> = visit_counts.iter().map(|(&pos, &visits)| (pos, visits)).collect();
        sorted_visits.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by visits descending
        log::info!("🔍 DEBUG Visit Counts after {} simulations:", num_simulations);
        log::info!("   Best position: {} with {} visits", best_position, max_visits);
        log::info!("   Top 5 positions: {:?}", sorted_visits.iter().take(5).map(|(pos, v)| format!("pos{}:{}", pos, v)).collect::<Vec<_>>());
        let visit_values: Vec<usize> = sorted_visits.iter().map(|(_, v)| *v).collect();
        let avg_visits = visit_values.iter().sum::<usize>() as f64 / visit_values.len() as f64;
        log::info!("   Visit avg: {:.1}, max: {}, min: {}", avg_visits, visit_values[0], visit_values[visit_values.len()-1]);
    }

    // Create distribution based on visit counts
    let mut distribution = vec![0.0f32; plateau.tiles.len()];
    let total_visits: usize = visit_counts.values().sum();

    if total_visits > 0 {
        for &pos in &legal_moves {
            let visits = visit_counts[&pos];
            distribution[pos] = visits as f32 / total_visits as f32;
        }
    }

    let policy_distribution = Tensor::from_slice(&distribution);
    let avg_value = total_values[&best_position] / visit_counts[&best_position].max(1) as f64;

    MCTSResult {
        best_position,
        board_tensor: input_tensor,
        subscore: avg_value,
        policy_distribution: policy_distribution.shallow_clone(),
        policy_distribution_boosted: policy_distribution,
        boost_intensity: 0.0,
        graph_features: None,
        plateau: Some(plateau.clone()),
        current_turn: Some(current_turn),
        total_turns: Some(total_turns),
        q_value_distribution: None,
    }
}

// ============================================================================
// HEX-AWARE HEURISTIC HELPERS (No rollouts!)
// ============================================================================

/// Calculate current score for a plateau state
fn calculate_score_for_plateau(plateau: &Plateau) -> i32 {
    result(plateau)
}

/// Calculate line completion potential: how close are we to completing valuable lines?
/// Returns a score [0-100] based on lines that are 2/3+ filled with same value
fn calculate_line_completion_potential(plateau: &Plateau, _current_turn: usize, _total_turns: usize) -> f64 {
    use crate::neural::tensor_conversion::LINE_DEFS;

    let mut potential = 0.0;

    for (positions, orientation) in LINE_DEFS {
        if positions.len() < 3 {
            continue; // Need at least 3 tiles for scoring
        }

        let mut counts = [0usize; 10]; // Count tiles 0-9
        let mut filled = 0usize;

        for &pos in *positions {
            let tile = plateau.tiles[pos];
            if tile == Tile(0, 0, 0) {
                continue; // Empty
            }
            filled += 1;
            let value = match orientation {
                0 => tile.0,
                1 => tile.1,
                2 => tile.2,
                _ => 0,
            };
            if value > 0 && (value as usize) < counts.len() {
                counts[value as usize] += 1;
            }
        }

        // Check if line is 2/3+ filled
        let fill_ratio = filled as f64 / positions.len() as f64;
        if fill_ratio >= 0.66 {
            // Find max count
            let max_count = counts.iter().max().copied().unwrap_or(0);
            if max_count >= 2 {
                // Line has potential! Weight by fill ratio and max count
                let value_score = match max_count {
                    2 => 5.0,   // Potential for small line
                    3 => 15.0,  // Potential for medium line
                    4 => 30.0,  // Potential for large line
                    _ => 50.0,  // Potential for full line
                };
                potential += value_score * fill_ratio;
            }
        }
    }

    potential
}

/// Get hex degree: number of lines passing through a position
/// Center: 6 lines, Mid-ring: 4-5 lines, Edges: 2-3 lines
fn get_hex_degree(position: usize) -> usize {
    use crate::neural::tensor_conversion::LINE_DEFS;

    let mut degree = 0;
    for (positions, _) in LINE_DEFS {
        if positions.contains(&position) {
            degree += 1;
        }
    }
    degree
}
