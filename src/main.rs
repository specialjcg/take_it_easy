use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use chrono::Utc;
use rayon::iter::ParallelIterator;
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use futures_util::stream::SplitSink;
use rand::{Rng, rng};
use rayon::prelude::IntoParallelIterator;
use serde_json;
use tch::{Device, IndexOp, nn, Tensor};
use tch::nn::{Optimizer, OptimizerConfig};
use tokio::net::TcpListener;
use tokio::{spawn, task};
use tokio::time::sleep;
use tokio_tungstenite::{accept_async, WebSocketStream};
use tokio_tungstenite::tungstenite::protocol::Message;

use create_plateau_empty::create_plateau_empty;
use create_shuffle_deck::create_shuffle_deck;
use result::result;
use crate::logging::setup_logging;
use crate::mcts_vs_human::play_mcts_vs_human;
use crate::policy_value_net::{PolicyNet, ValueNet};
use crate::remove_tile_from_deck::{remove_tile_from_deck, replace_tile_in_deck};
use crate::test::{Deck, Plateau, Tile};

mod test;
mod result;
mod remove_tile_from_deck;
mod create_plateau_empty;
mod create_shuffle_deck;

mod policy_value_net;
mod mcts_vs_human;
mod logging;

fn generate_tile_image_names(tiles: &[Tile]) -> Vec<String> {
    tiles.iter().map(|tile| {
        format!("../image/{}{}{}.png", tile.0, tile.1, tile.2)
    }).collect()
}

#[derive(Parser, Debug)]
#[command(name = "take_it_easy")]
struct Config {
    /// Number of games to simulate
    #[arg(short = 'g', long, default_value_t = 100)]
    num_games: usize,

    /// Number of simulations per game state
    #[arg(short = 's', long, default_value_t = 100)]
    num_simulations: usize,

    /// Run MCTS vs Human instead of training
    #[arg(long)]
    mcts_vs_human: bool,
}


#[tokio::main]
async fn main() {
    let config = Config::parse();
    let model_path = "model_weights";
    setup_logging();

    // Initialize VarStore
    let mut vs_policy = nn::VarStore::new(Device::Cpu);
    let mut vs_value = nn::VarStore::new(Device::Cpu);
    let input_dim = (5, 47, 1);
    let mut policy_net = PolicyNet::new(&vs_policy, input_dim);
    let mut value_net = ValueNet::new(&mut vs_value, input_dim);





    // Load weights if the model directory exists
    if Path::new(model_path).exists() {
        log::info!("🔄 Loading model weights from {}", model_path);

        if let Err(e) = policy_net.load_model(&mut vs_policy, "model_weights/policy/policy.params") {
            log::error!("⚠️ Error loading PolicyNet: {:?}", e);
            log::info!("➡️  Initializing PolicyNet with random weights.");
        }

        if let Err(e) = value_net.load_model(&mut vs_value, "model_weights/value/value.params") {
            log::error!("⚠️ Error loading ValueNet: {:?}", e);
            log::info!("➡️  Initializing ValueNet with random weights.");
        }
    } else {
        log::info!("📭 No pre-trained model found. Initializing new models.");
    }
    let mut optimizer_policy = nn::Adam ::default().build(&vs_policy, 1e-3).unwrap();
    let mut optimizer_value = nn::Adam {
        wd: 1e-5, // 🔥 Lower weight decay to prevent excessive penalization
        ..Default::default()
    }.build(&vs_value, 1e-3).unwrap(); // 🔥 Higher learning rate to accelerate learning





    // ➕ Duel Mode: MCTS vs Human
    if config.mcts_vs_human {
        let listener = TcpListener::bind("127.0.0.1:9001")
            .await
            .expect("Unable to bind WebSocket on port 9001 for MCTS vs Human");

        log::info!("🧍‍♂️🤖 Waiting for MCTS vs Human connection...");
        let (stream, _) = listener.accept().await.unwrap();
        let ws_stream = accept_async(stream).await.unwrap();
        let (mut write, mut read) = ws_stream.split();

        play_mcts_vs_human(
            &policy_net,
            &value_net,
            config.num_simulations,
            &mut write,
            &mut read,
        )
            .await;

        return; // Exit after duel game
    }

    // 🧠 Training Mode
    let listener = TcpListener::bind("127.0.0.1:9000")
        .await
        .expect("Unable to bind WebSocket on port 9000 for training");
    log::info!("🧠 Training WebSocket server started at ws://127.0.0.1:9000");

    train_and_evaluate(
        &vs_policy,
        &vs_value,
        &mut policy_net,
        &mut value_net,
        &mut optimizer_policy,
        &mut optimizer_value,
        config.num_games,
        config.num_simulations,
        50, // Evaluate every 50 games
        listener.into(),
    )
        .await;
}


/// Finds the best move using MCTS with neural network guidance
fn mcts_find_best_position_for_tile_with_nn(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    num_simulations: usize,
    current_turn:usize,
    total_turns:usize,
) -> MCTSResult {
    let legal_moves = get_legal_moves(plateau.clone());
    if legal_moves.is_empty() {
        return MCTSResult {
            best_position: 0,
            board_tensor: convert_plateau_to_tensor(plateau, &chosen_tile, deck,current_turn, total_turns),
            subscore: 0.0,
        };
    }

    let board_tensor = convert_plateau_to_tensor(plateau, &chosen_tile, deck,current_turn, total_turns);
    let policy_logits = policy_net.forward(&board_tensor, false);
    let policy = policy_logits.log_softmax(-1, tch::Kind::Float).exp(); // Log-softmax improves numerical stability
    let dummy_input = Tensor::randn(&[1, 5, 47, 1], (tch::Kind::Float, tch::Device::Cpu));
    let output = value_net.forward(&dummy_input, false);
    log::info!("Dummy output ValueNet: {:.4}", output.double_value(&[]));
    let mut visit_counts: HashMap<usize, usize> = HashMap::new();
    let mut total_scores: HashMap<usize, f64> = HashMap::new();
    let mut ucb_scores: HashMap<usize, f64> = HashMap::new();
    let mut total_visits: i32 = 0;


    for &position in &legal_moves {
        visit_counts.insert(position, 0);
        total_scores.insert(position, 0.0);
        ucb_scores.insert(position, f64::NEG_INFINITY);
    }

    let c_puct = 3.5;

    // **Compute ValueNet scores for all legal moves**
    let mut value_estimates = HashMap::new();
    let mut min_value = f64::INFINITY;
    let mut max_value = f64::NEG_INFINITY;

    for &position in &legal_moves {
        let mut temp_plateau = plateau.clone();
        let mut temp_deck = deck.clone();

        temp_plateau.tiles[position] = chosen_tile;
        temp_deck = replace_tile_in_deck(&temp_deck, &chosen_tile);
        let board_tensor_temp = convert_plateau_to_tensor(&temp_plateau, &chosen_tile, &temp_deck,current_turn, total_turns);

        let pred_value = value_net.forward(&board_tensor_temp, false).double_value(&[]);
        let pred_value = pred_value.clamp(-1.0, 1.0);

        // Track min and max for dynamic pruning
        min_value = min_value.min(pred_value);
        max_value = max_value.max(pred_value);

        value_estimates.insert(position, pred_value);
    }

    // **Dynamic Pruning Strategy**
    let value_threshold = min_value + (max_value - min_value) * 0.2; // Keep top 80% moves

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

        let subset_moves: Vec<usize> = moves_with_prior.iter().take(top_k).map(|&(pos, _)| pos).collect();

        for &position in &subset_moves {
            let mut temp_plateau = plateau.clone();
            let mut temp_deck = deck.clone();

            temp_plateau.tiles[position] = chosen_tile;
            temp_deck = replace_tile_in_deck(&temp_deck, &chosen_tile);

            let value_estimate = *value_estimates.get(&position).unwrap_or(&0.0);

            // **Improved Adaptive Rollout Strategy**
            let rollout_count = match value_estimate {
                x if x > 8.0 => 2, // Very strong move -> minimal rollouts
                x if x > 6.0 => 4, // Strong move -> fewer rollouts
                x if x > 4.0 => 6, // Decent move -> moderate rollouts
                _ => 8,           // Uncertain move -> more rollouts
            };

            let mut total_simulated_score = 0.0;

            for _ in 0..rollout_count {
                let mut lookahead_plateau = temp_plateau.clone();
                let mut lookahead_deck = temp_deck.clone();

                // 🔮 Étape 1.1 — Tirer une tuile hypothétique (T2)
                if lookahead_deck.tiles.is_empty() {
                    continue;
                }
                let tile2_index = rand::thread_rng().gen_range(0..lookahead_deck.tiles.len());
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
            *total_score += simulated_score as f64;

            let exploration_param = c_puct * (total_visits as f64).ln() / (1.0 + *visits as f64);
            let prior_prob = policy.i((0, position as i64)).double_value(&[]);
            let average_score = *total_score / (*visits as f64);
            // 🧪 Reduce weight of rollout average
            let mut ucb_score = (average_score * 0.5) // reduce rollout influence
                + exploration_param * (prior_prob.sqrt())
                + 0.25 * value_estimate.clamp(0.0, 2.0);


            // 🔥 Explicit Priority Logic HERE 🔥
            // 1️⃣ Ajoute cette fonction en dehors de ta mcts_find_best_position_for_tile_with_nn


            // 2️⃣ Intègre ceci dans ta boucle ucb_scores, juste après le boost fixe

            if chosen_tile.0 == 9 && [7, 8, 9, 10, 11].contains(&position) {
                ucb_score += 10000.0;  // double boost
            } else if chosen_tile.0 == 5 && [3, 4, 5, 6, 12, 13, 14, 15].contains(&position) {
                ucb_score += 8000.0;
            } else if chosen_tile.0 == 1 && [0, 1, 2, 16, 17, 18].contains(&position) {
                ucb_score += 6000.0;
            }

            // 🔥 Alignment Priority Logic 🔥





            ucb_scores.insert(position, ucb_score);
        }
    }

    // Select the move with the highest UCB score
    let best_position = legal_moves.into_iter()
        .max_by(|&a, &b| {
            ucb_scores.get(&a).unwrap_or(&f64::NEG_INFINITY)
                .partial_cmp(ucb_scores.get(&b).unwrap_or(&f64::NEG_INFINITY))
                .unwrap_or(std::cmp::Ordering::Equal)
        }).unwrap_or(0);


    // **NEW: Simulate the Rest of the Game to Get Final Score**
    let mut final_plateau = plateau.clone();
    let mut final_deck = deck.clone();
    final_plateau.tiles[best_position] = chosen_tile;
    final_deck = replace_tile_in_deck(&final_deck, &chosen_tile);

    while !is_plateau_full(&final_plateau) {
        let tile_index = rand::thread_rng().gen_range(0..final_deck.tiles.len());
        let random_tile = final_deck.tiles[tile_index];

        let available_moves = get_legal_moves(final_plateau.clone());
        if available_moves.is_empty() {
            break;
        }

        let random_position = available_moves[rand::thread_rng().gen_range(0..available_moves.len())];
        final_plateau.tiles[random_position] = random_tile;
        final_deck = replace_tile_in_deck(&final_deck, &random_tile);
    }

    let final_score = result(&final_plateau); // Get actual game score
    let prior_prob = policy.i((0, best_position as i64)).double_value(&[]);
    let value_estimate = value_net.forward(&board_tensor, false).double_value(&[]);

    log::info!(
        "🤖 MCTS chose position {} | Policy Prior: {:.4} | ValueNet Estimate: {:.2} | Final Simulated Score: {:.2}",
        best_position, prior_prob, value_estimate, final_score
    );
    MCTSResult {
        best_position,
        board_tensor,
        subscore: final_score as f64, // Store real final score, not UCB score
    }
}
fn local_lookahead(mut plateau: Plateau, mut deck: Deck, depth: usize) -> i32 {
    for _ in 0..depth {
        if is_plateau_full(&plateau) || deck.tiles.is_empty() {
            break;
        }

        let tile_index = rand::thread_rng().gen_range(0..deck.tiles.len());
        let chosen_tile = deck.tiles[tile_index];

        let legal_moves = get_legal_moves(plateau.clone());
        if legal_moves.is_empty() {
            break;
        }

        let best_pos = legal_moves
            .into_iter()
            .max_by_key(|&pos| compute_alignment_score(&plateau, pos, &chosen_tile) as i32)
            .unwrap();

        plateau.tiles[best_pos] = chosen_tile;
        deck = replace_tile_in_deck(&deck, &chosen_tile);
    }

    result(&plateau)
}
fn compute_global_stats(game_data: &[MCTSResult]) -> (Tensor, Tensor) {
    let stacked = Tensor::cat(
        &game_data.iter().map(|gd| gd.board_tensor.shallow_clone()).collect::<Vec<_>>(),
        0
    );

    let mean = stacked.mean_dim(&[0i64, 2, 3][..], true, tch::Kind::Float);
    let std = stacked.std_dim(&[0i64, 2, 3][..], true, true).clamp_min(1e-8);


    (mean, std)
}

fn compute_alignment_score(plateau: &Plateau, position: usize, tile: &Tile) -> f64 {
    let patterns: Vec<(&[usize], Box<dyn Fn(&Tile) -> i32>)> = vec![
        (&[0, 1, 2], Box::new(|t: &Tile| t.0)),
        (&[3, 4, 5, 6], Box::new(|t: &Tile| t.0)),
        (&[7, 8, 9, 10, 11], Box::new(|t: &Tile| t.0)),
        (&[12, 13, 14, 15], Box::new(|t: &Tile| t.0)),
        (&[16, 17, 18], Box::new(|t: &Tile| t.0)),
        (&[0, 3, 7], Box::new(|t: &Tile| t.1)),
        (&[1, 4, 8, 12], Box::new(|t: &Tile| t.1)),
        (&[2, 5, 9, 13, 16], Box::new(|t: &Tile| t.1)),
        (&[6, 10, 14, 17], Box::new(|t: &Tile| t.1)),
        (&[11, 15, 18], Box::new(|t: &Tile| t.1)),
        (&[7, 12, 16], Box::new(|t: &Tile| t.2)),
        (&[3, 8, 13, 17], Box::new(|t: &Tile| t.2)),
        (&[0, 4, 9, 14, 18], Box::new(|t: &Tile| t.2)),
        (&[1, 5, 10, 15], Box::new(|t: &Tile| t.2)),
        (&[2, 6, 11], Box::new(|t: &Tile| t.2)),
    ];

    let mut score = 0.0;

    for (indices, selector) in patterns {
        if indices.contains(&position) {
            // Récupère les valeurs dans le pattern
            let values: Vec<i32> = indices
                .iter()
                .map(|&i| selector(&plateau.tiles[i]))
                .filter(|&v| v != 0) // Ignore les cases vides
                .collect();

            if !values.is_empty() {
                // Moyenne ou somme des alignements existants
                let sum = values.iter().sum::<i32>() as f64;
                score += sum / values.len() as f64;
            }
        }
    }

    score
}


fn normalize_input(tensor: &Tensor, global_mean: &Tensor, global_std: &Tensor) -> Tensor {
    (tensor - global_mean) / (global_std + 1e-8)
}


fn train_network_with_game_data(
    vs_policy: &nn::VarStore,
    vs_value: &nn::VarStore,
    game_data: &[MCTSResult],
    discount_factor: f64,
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    optimizer_policy: &mut Optimizer,
    optimizer_value: &mut Optimizer,
) {
    // Hyperparameters
    let entropy_weight = 0.05;
    let gamma = 0.99;
    let epsilon = 1e-8;

    // Initialize accumulators
    let mut predictions = Vec::new();
    let mut targets = Vec::new();
    let mut total_policy_loss = Tensor::zeros(&[], tch::kind::FLOAT_CPU);
    let mut total_value_loss = Tensor::zeros(&[], tch::kind::FLOAT_CPU);
    let mut total_entropy_loss = Tensor::zeros(&[], tch::kind::FLOAT_CPU);

    // Initialize trajectory rewards and discounted sum
    let mut trajectory_rewards = Vec::new();
    let mut discounted_sum = Tensor::zeros(&[], (tch::Kind::Float, tch::Device::Cpu));

    // === Training Loop ===
    for (step, result) in game_data.iter().rev().enumerate() {
        // 🛑 No Normalization: Use raw tensor
        let state = result.board_tensor.shallow_clone();
        let mean = state.mean(tch::Kind::Float);
        let std = state.std(true);
        let normalized_state = (state - mean) / (std + 1e-8);  // Avoid division by zero
        // Forward pass through networks with normalized state
        let pred_policy = policy_net.forward(&normalized_state, true).clamp_min(1e-7);
        let pred_value = value_net.forward(&normalized_state, true);

        // Forward pass through networks with normalized state
        // Normalize reward: divide by a constant max value (e.g., 100)
        let reward = Tensor::from(result.subscore).to_kind(tch::Kind::Float) / 100.0;
        let gamma_tensor = Tensor::of_slice(&[gamma]).to_kind(tch::Kind::Float);

        // ✅ NaN & Inf Check for reward
        // ✅ NaN & Inf Check for reward
        if reward.isnan().any().double_value(&[]) > 0.0 || reward.isinf().any().double_value(&[]) > 0.0 {
            log::error!("⚠️ NaN or Inf detected in reward at step {}", step);
            continue;
        }

        // Update discounted sum with normalized reward
        discounted_sum = reward + gamma_tensor * discounted_sum;

        // ✅ NaN & Inf Check for discounted sum
        if discounted_sum.isnan().any().double_value(&[]) > 0.0 || discounted_sum.isinf().any().double_value(&[]) > 0.0 {
            log::error!("⚠️ NaN or Inf detected in discounted sum at step {}", step);
            continue;
        }

        // Store the value for analysis
        trajectory_rewards.push(discounted_sum.double_value(&[]));

        // Generate target tensor directly from discounted sum
        let discounted_reward = discounted_sum.shallow_clone();

        // Compute the error for logging

        // Log normalized values for better tracking
        if step % 50 == 0 {
            log::info!(
            "🔍 Step {} | Normalized Value Prediction: {:.4} | Normalized Target: {:.4} | Error: {:.4}",
            step,
            pred_value.double_value(&[]),
            discounted_sum.double_value(&[]),
            (pred_value.double_value(&[]) - discounted_sum.double_value(&[])).abs()
        );
        }

        // Append for later analysis
        predictions.push(pred_value.double_value(&[]));
        targets.push(discounted_reward.double_value(&[]));

        // === Compute Losses ===
        // Policy loss
        let best_position = result.best_position as i64;
        let target_policy = Tensor::zeros(&[1, pred_policy.size()[1]], tch::kind::FLOAT_CPU);
        target_policy.i((0, best_position)).fill_(1.0);
        let log_policy = pred_policy.log();
        let policy_loss = -(target_policy * log_policy.shallow_clone()).sum(tch::Kind::Float);
        total_policy_loss += policy_loss;

        // Entropy loss
        let entropy_loss = -(pred_policy * (log_policy + epsilon)).sum(tch::Kind::Float);
        total_entropy_loss += entropy_loss;

        // Value loss (Smooth L1 loss)
        let delta = 1.0;
        let diff = discounted_reward.shallow_clone() - pred_value.shallow_clone();
        let value_loss = diff.abs().clamp_max(delta).pow_tensor_scalar(2.0) * 0.5
            + (diff.abs() - delta).clamp_min(0.0) * delta;
        total_value_loss += value_loss.mean(tch::Kind::Float);

        log::info!("🎯 ValueNet Prediction: {:.2} | Target: {:.2} | Error: {:.2}", pred_value.double_value(&[]), discounted_reward.double_value(&[]), (pred_value.double_value(&[]) - discounted_reward.double_value(&[])).abs());
    }

    // === Backpropagation ===
    let total_loss: Tensor = total_policy_loss.shallow_clone()
        + total_value_loss.shallow_clone()
        + (entropy_weight * total_entropy_loss.shallow_clone());

    // Log the loss before backpropagation
    log::info!("💡 Total Loss before backward: {:.4}", total_loss.double_value(&[]));

    // ✅ NaN and Inf check before backpropagation
    if total_loss.isnan().any().double_value(&[]) > 0.0 {
        log::error!("⚠️ NaN detected in total loss! Skipping backpropagation.");
        return;
    }
    if total_loss.isinf().any().double_value(&[]) > 0.0 {
        log::error!("⚠️ Inf detected in total loss! Skipping backpropagation.");
        return;
    }

    total_loss.backward();

    tch::no_grad(|| {
        for (_, tensor) in vs_value.variables() {
            if tensor.grad().defined() {
                tensor.grad().clamp_(-5.0, 5.0);
            }
        }
    });

    // === Optimizer Step ===
    optimizer_policy.step();
    optimizer_policy.zero_grad();
    optimizer_value.step();
    optimizer_value.zero_grad();

    log::info!(
        "🎯 Update Complete | Policy Loss: {:.4}, Value Loss: {:.4}, Entropy Loss: {:.4}",
        total_policy_loss.double_value(&[]),
        total_value_loss.double_value(&[]),
        total_entropy_loss.double_value(&[])
    );
}







fn convert_plateau_to_tensor(plateau: &Plateau, tile: &Tile, deck: &Deck, current_turn: usize, total_turns: usize) -> Tensor {
    let mut features = vec![0.0; 5 * 47]; // 5 channels: Plateau, Tile, Deck, Score Potential, Turn Indicator

    // Channel 1-3: Plateau, Tile, Deck (Déjà implémenté)
    for (i, t) in plateau.tiles.iter().enumerate() {
        if i < 19 {
            features[i] = (t.0 as f32 / 10.0).clamp(0.0, 1.0);
            features[47 + i] = (t.1 as f32 / 10.0).clamp(0.0, 1.0);
            features[2 * 47 + i] = (t.2 as f32 / 10.0).clamp(0.0, 1.0);
        }
    }

    // Channel 4: **Score Potentiel pour chaque position**
    let potential_scores = compute_potential_scores(plateau);
    for i in 0..19 {
        features[3 * 47 + i] = potential_scores[i];
    }

    // Channel 5: **Tour Actuel**
    let turn_normalized = current_turn as f32 / total_turns as f32;
    for i in 0..19 {
        features[4 * 47 + i] = turn_normalized;
    }

    // Convertir en tensor PyTorch
    Tensor::of_slice(&features).view([1, 5, 47, 1])
}
fn compute_potential_scores(plateau: &Plateau) -> Vec<f32> {
    let mut scores = vec![0.0; 19]; // Potential score for each position

    let patterns: Vec<(&[usize], i32, Box<dyn Fn(&Tile) -> i32>)> = vec![
        (&[0, 1, 2], 3, Box::new(|tile: &Tile| tile.0)),
        (&[3, 4, 5, 6], 4, Box::new(|tile: &Tile| tile.0)),
        (&[7, 8, 9, 10, 11], 5, Box::new(|tile: &Tile| tile.0)),
        (&[12, 13, 14, 15], 4, Box::new(|tile: &Tile| tile.0)),
        (&[16, 17, 18], 3, Box::new(|tile: &Tile| tile.0)),
        (&[0, 3, 7], 3, Box::new(|tile: &Tile| tile.1)),
        (&[1, 4, 8, 12], 4, Box::new(|tile: &Tile| tile.1)),
        (&[2, 5, 9, 13, 16], 5, Box::new(|tile: &Tile| tile.1)),
        (&[6, 10, 14, 17], 4, Box::new(|tile: &Tile| tile.1)),
        (&[11, 15, 18], 3, Box::new(|tile: &Tile| tile.1)),
        (&[7, 12, 16], 3, Box::new(|tile: &Tile| tile.2)),
        (&[3, 8, 13, 17], 4, Box::new(|tile: &Tile| tile.2)),
        (&[0, 4, 9, 14, 18], 5, Box::new(|tile: &Tile| tile.2)),
        (&[1, 5, 10, 15], 4, Box::new(|tile: &Tile| tile.2)),
        (&[2, 6, 11], 3, Box::new(|tile: &Tile| tile.2)),
    ];

    for (indices, multiplier, selector) in &patterns {
        let mut filled_values = Vec::new();
        let mut empty_positions = Vec::new();

        for &pos in *indices {
            if plateau.tiles[pos] == Tile(0, 0, 0) {
                empty_positions.push(pos);
            } else {
                filled_values.push(selector(&plateau.tiles[pos]) as f32);
            }
        }

        // If at least one tile is placed in the pattern
        if !filled_values.is_empty() {
            let avg_filled_value = filled_values.iter().sum::<f32>() / filled_values.len() as f32;
            let potential_score = avg_filled_value * (*multiplier as f32);

            for &pos in empty_positions.iter() {
                scores[pos] += potential_score / empty_positions.len() as f32; // Distribute potential score
            }
        }
    }

    scores
}










pub fn load_game_data(file_path: &str) -> Vec<MCTSResult> {
    // Paths for the .pt files
    let states_path = format!("{}_states.pt", file_path);
    let positions_path = format!("{}_positions.pt", file_path);
    let subscores_path = format!("{}_subscores.pt", file_path);

    // Check if all files exist
    if !Path::new(&states_path).exists() {
        println!("⚠️  Warning: '{}' not found. Returning empty dataset.", states_path);
        return Vec::new();
    }
    if !Path::new(&positions_path).exists() {
        println!("⚠️  Warning: '{}' not found. Returning empty dataset.", positions_path);
        return Vec::new();
    }
    if !Path::new(&subscores_path).exists() {
        println!("⚠️  Warning: '{}' not found. Returning empty dataset.", subscores_path);
        return Vec::new();
    }

    println!("🚀 Loading game data from .pt files...");

    // Load the saved tensors
    let state_tensor = Tensor::load(states_path).expect("Failed to load states");
    let position_tensor = Tensor::load(positions_path).expect("Failed to load positions");
    let subscore_tensor = Tensor::load(subscores_path).expect("Failed to load subscores");

    // Convert them back into MCTSResult objects
    let mut data = Vec::new();
    for i in 0..state_tensor.size()[0] {
        data.push(MCTSResult {
            board_tensor: state_tensor.get(i),
            best_position: position_tensor.get(i).int64_value(&[]) as usize,
            subscore: subscore_tensor.get(i).double_value(&[]),
        });
    }
    println!("✅ Loaded {} game records.", data.len());
    data
}




fn deserialize_game_data(line: &str) -> Option<MCTSResult> {
    let parts: Vec<&str> = line.split(',').collect();

    if parts.len() != 3 {
        log::error!("Invalid data format: '{}'", line);
        return None;
    }

    // Parse tensor
    let state_values: Vec<f32> = parts[0]
        .split_whitespace()
        .map(|v| v.parse::<f32>())
        .collect::<Result<Vec<f32>, _>>()
        .unwrap_or_else(|_| {
            log::error!("Failed to parse state tensor in line '{}'", line);
            vec![]
        });

    if state_values.len() != 5 * 47 {
        log::error!("ERROR: Parsed tensor has incorrect size {} (expected 235). Data: '{}'", state_values.len(), parts[0]);
        return None;
    }

    let state_tensor = Tensor::of_slice(&state_values).view([1, 5, 47, 1]);

    // Parse subscore
    let subscore = parts[1].parse::<f64>().unwrap_or_else(|_| {
        log::error!("Failed to parse subscore in line '{}'", line);
        0.0
    });

    // Parse best position
    let best_position = parts[2].parse::<usize>().unwrap_or_else(|_| {
        log::error!("Failed to parse best_position in line '{}'", line);
        0
    });

    Some(MCTSResult {
        board_tensor: state_tensor,
        subscore,
        best_position,
    })
}




fn save_game_data(file_path: &str, game_data: Vec<MCTSResult>) {
    println!("🚀 Saving game data to .pt files...");

    let mut tensors = vec![];
    let mut positions = vec![];
    let mut subscores = vec![];

    for result in game_data {
        tensors.push(result.board_tensor.shallow_clone());
        positions.push(result.best_position as i64);
        subscores.push(result.subscore as f32);
    }

    // Création des nouveaux tensors
    let state_tensor = Tensor::stack(&tensors, 0);
    let position_tensor = Tensor::of_slice(&positions).view([-1, 1]);
    let subscore_tensor = Tensor::of_slice(&subscores).view([-1, 1]);

    // 🔄 Append logic: charger les anciens tensors s'ils existent
    let combined_states = if let Ok(prev) = Tensor::load(format!("{}_states.pt", file_path)) {
        Tensor::cat(&[prev, state_tensor], 0)
    } else {
        state_tensor
    };

    let combined_positions = if let Ok(prev) = Tensor::load(format!("{}_positions.pt", file_path)) {
        Tensor::cat(&[prev, position_tensor], 0)
    } else {
        position_tensor
    };

    let combined_subscores = if let Ok(prev) = Tensor::load(format!("{}_subscores.pt", file_path)) {
        Tensor::cat(&[prev, subscore_tensor], 0)
    } else {
        subscore_tensor
    };

    // 🔄 Sauvegarde des tensors concaténés
    if let Err(e) = combined_states.save(format!("{}_states.pt", file_path)) {
        log::info!("❌ Error saving states: {:?}", e);
    }
    if let Err(e) = combined_positions.save(format!("{}_positions.pt", file_path)) {
        log::info!("❌ Error saving positions: {:?}", e);
    }
    if let Err(e) = combined_subscores.save(format!("{}_subscores.pt", file_path)) {
        log::info!("❌ Error saving subscores: {:?}", e);
    }

    log::info!("✅ Save complete!");
}


fn tensor_to_vec(tensor: &Tensor) -> Vec<f32> {
    // Flatten the tensor into a 1D array
    let flattened = tensor.view(-1); // Reshape the tensor to a 1D view

    // Convert each value in the flattened tensor to f32 and collect into a Vec
    let mut vec = Vec::new();
    for i in 0..flattened.size()[0] {
        let value = flattened.i(i).double_value(&[]) as f32;
        vec.push(value);
    }

    vec
}


fn serialize_tensor(tensor: Tensor) -> String {
    let data: Vec<f32> = tensor_to_vec(&tensor); // Converts the slice to a Vec<f32>
    data.iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}
#[derive(Debug)]
pub struct MCTSResult {
    pub board_tensor: Tensor,
    pub best_position: usize,
    pub subscore: f64,
}

impl Clone for MCTSResult {
    fn clone(&self) -> Self {
        MCTSResult {
            board_tensor: self.board_tensor.shallow_clone(), // Manually clone the tensor
            best_position: self.best_position,
            subscore: self.subscore,
        }
    }
}

fn append_to_results_file(file_path: &str,  avg_score: f64) {
    let timestamp = Utc::now().to_rfc3339();
    let result_line = format!("{},{:.2}\n",timestamp,  avg_score );

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)
        .expect("Unable to open results file");
    let mut writer = BufWriter::new(file);
    writer
        .write_all(result_line.as_bytes())
        .expect("Unable to write to results file");
}
async fn reconnect_websocket(
    listener: &TcpListener,
) -> Option<SplitSink<WebSocketStream<tokio::net::TcpStream>, Message>> {
    match listener.accept().await {
        Ok((stream, _)) => {
            log::info!("Re-establishing WebSocket connection...");
            let ws_stream = accept_async(stream).await.expect("Failed to accept WebSocket");
            let (write, _) = ws_stream.split();
            Some(write)
        }
        Err(e) => {
            log::error!("Error while reconnecting WebSocket: {:?}", e);
            None
        }
    }
}
async fn train_and_evaluate(
    vs_policy: &nn::VarStore,
    vs_value: &nn::VarStore,
    policy_net: &mut PolicyNet,
    value_net: &mut ValueNet,
    optimizer_policy: &mut Optimizer,
    optimizer_value: &mut Optimizer,
    num_games: usize,
    num_simulations: usize,
    evaluation_interval: usize,
    listener: Arc<TcpListener>,
) {
    let mut total_score = 0;
    let mut games_played = 0;
    let results_file = "results.csv";

    while let Ok((stream, _)) = listener.accept().await {
        let ws_stream = accept_async(stream).await.expect("Failed to accept WebSocket");
        let (mut write, _) = ws_stream.split();
        let mut scores_by_position: HashMap<usize, Vec<i32>> = HashMap::new();
        let mut scores = Vec::new(); // Stocke les scores
        let evaluation_interval_average = 10;

        while games_played < num_games {

            log::info!(
                "Starting training iteration {}/{}...",
                games_played + 1,
                num_games
            );
            log::info!("\n🚀 Starting Batch {}", games_played / evaluation_interval + 1);

            let mut batch_games_played = 0; // Tracks games processed in this evaluation interval

            let max_memory_size = 1000; // Store last 500 games

            for game in 0..evaluation_interval {
                let mut deck = create_shuffle_deck();
                let mut plateau = create_plateau_empty();
                let mut game_data = Vec::new();
                let mut first_move: Option<(usize, Tile)> = None;
                let total_turns = 19; // The number of moves in the game
                let mut current_turn = 0;
                while !is_plateau_full(&plateau) {
                    let tile_index = rng().random_range(0..deck.tiles.len());
                    let chosen_tile = deck.tiles[tile_index];
                    // ✅ **Send preview before placement**
                    // ✅ **INSERT YOUR NEW CODE HERE**
                    let chosen_tile_image = format!("../image/{}{}{}.png", chosen_tile.0, chosen_tile.1, chosen_tile.2);
                    let payload = serde_json::json!({
        "next_tile": chosen_tile_image,
        "plateau_tiles": generate_tile_image_names(&plateau.tiles)
    });
                    let serialized = serde_json::to_string(&payload).unwrap();
                    write.send(Message::Text(serialized)).await.unwrap();

                    let game_result = mcts_find_best_position_for_tile_with_nn(
                        &mut plateau,
                        &mut deck,
                        chosen_tile,
                        policy_net,
                        value_net,
                        num_simulations,
                        current_turn,
                        total_turns,
                    );

                    let best_position = game_result.best_position;
                    if first_move.is_none() {
                        first_move = Some((best_position, chosen_tile));
                    }
                    plateau.tiles[best_position] = chosen_tile;
                    deck = replace_tile_in_deck(&deck, &chosen_tile);
                    // ✅ INSERT THIS TO SEND SCORE TO CLIENT
                    let current_score = result(&plateau);
                    let score_payload = serde_json::json!({
    "type": "score_update",
    "current_score": current_score,
});
                    let serialized_score = serde_json::to_string(&score_payload).unwrap();
                    if let Err(e) = write.send(Message::Text(serialized_score)).await {
                        log::error!("WebSocket error when sending score: {:?}", e);
                        if let Some(new_write) = reconnect_websocket(&listener).await {
                            write = new_write;
                        } else {
                            log::error!("Failed to reconnect WebSocket. Exiting...");
                            break;
                        }
                    }

                    game_data.push(game_result); // Store training data

                    // ✅ **INSERT YOUR NEW CODE HERE**
                    let payload_after_placement = serde_json::json!({
        "next_tile": null, // Clear preview
        "plateau_tiles": generate_tile_image_names(&plateau.tiles) // new updated state
    });
                    let serialized = serde_json::to_string(&payload_after_placement).unwrap();

                    // ✅ Handle WebSocket disconnections
                    if let Err(e) = write.send(Message::Text(serialized.clone())).await {
                        log::error!("WebSocket error: {:?}. Attempting to reconnect...", e);

                        // **Reconnect WebSocket**
                        if let Some(new_write) = reconnect_websocket(&listener).await {
                            write = new_write;
                        } else {
                            log::error!("Failed to reconnect WebSocket. Exiting...");
                            break;
                        }
                    }
                    current_turn += 1; // Increment turn counter each time a tile is placed

                }

                let final_score = result(&plateau);

                if let Some((position, _)) = first_move {
                    scores_by_position
                        .entry(position)
                        .or_insert_with(Vec::new)
                        .push(final_score);
                }

                let mut batch_game_data = Vec::new();

                // Prioritized historical data
                let prioritized_data: Vec<MCTSResult> = load_game_data("game_data")
                    .into_iter().filter(|r| r.subscore > 100.0) // Only select high-score games
                    .take(50) // Limit to 50 samples to prevent overfitting
                    .collect();

                // Add historical data to batch
                batch_game_data.extend(prioritized_data);

                // Add current game's data to batch
                batch_game_data.extend(game_data.iter().map(|result| MCTSResult {
                    best_position: result.best_position,
                    board_tensor: result.board_tensor.shallow_clone(),
                    subscore: result.subscore,
                }));

                // Keep only last max_memory_size experiences
                if batch_game_data.len() > max_memory_size {
                    let to_remove = batch_game_data.len() - max_memory_size;
                    batch_game_data.drain(0..to_remove); // Remove oldest data
                }

                // Train in batches
                let batch_size = 10;
                for batch in batch_game_data.chunks(batch_size) {
                    train_network_with_game_data(
                        &vs_policy,
                        &vs_value,
                        batch, // Use each batch directly
                        final_score.into(),
                        policy_net,
                        value_net,
                        optimizer_policy,
                        optimizer_value,
                    );
                }

                log::info!("Game {} finished with score: {}", game + 1, final_score);
                scores.push(final_score);

                // Update batch-specific counters
                batch_games_played += 1;
                total_score += final_score;

                if game % evaluation_interval_average == 0 && game != 0 {
                    let moyenne: f64 = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
                    log::info!("📊 [Batch {}] Avg Score: {:.2} | Games Played: {}", games_played / evaluation_interval, moyenne, games_played);
                    log::info!("batch {} - Score moyen: {:.2}", game, moyenne);
                    write.send(Message::Text(format!("GAME_RESULT:{}", moyenne))).await.unwrap();
                }

                // Save current game data for future training
                save_game_data("game_data", game_data);
            }


            // Update main game counters
            games_played += batch_games_played;

            // Append results to the file
            let avg_score = total_score as f64 / games_played as f64;
            append_to_results_file(results_file,  avg_score);

            // Calculate and display averages
            let mut averages: Vec<(usize, f64)> = scores_by_position
                .iter()
                .map(|(position, scores)| {
                    let average_score: f64 =
                        scores.iter().sum::<i32>() as f64 / scores.len() as f64;
                    (*position, average_score)
                })
                .collect();

            averages.sort_by(|a, b| {
                b.1.partial_cmp(&a.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            log::info!("\n--- Average Scores by First Position (Sorted) ---");
            for (position, average_score) in averages {
                log::info!("Position: {}, Average Score: {:.2}", position, average_score);
            }

            // Evaluate model after each interval
            evaluate_model(policy_net, value_net,num_simulations).await;

            log::info!(
                "Games Played: {}, Total Score: {}, Avg Score: {:.2}",
                games_played,
                total_score,
                total_score as f32 / games_played as f32
            );
            let model_path = "model_weights";
            // Save model weights
            log::info!("Saving models to {}", model_path);
            log::info!("Saving model weights...");
            if let Err(e) = policy_net.save_model(vs_policy,"model_weights/policy/policy.params") {
                log::error!("Error saving PolicyNet weights: {:?}", e);
            }
            if let Err(e) = value_net.save_model(vs_value,"model_weights/value/value.params") {
                log::error!("Error saving ValueNet weights: {:?}", e);
            }
        }
        break; // Exit after handling one connection
    }
}

async fn evaluate_model(
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    num_simulations: usize,
) {
    log::info!("Evaluating model...");
    let mut scores = Vec::new();

    for _ in 0..10 {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        let total_turns = 19; // The number of moves in the game
        let mut current_turn = 0;
        while !is_plateau_full(&plateau) {
            let tile_index = rng().random_range(0..deck.tiles.len());
            let chosen_tile = deck.tiles[tile_index];
            let game_result = mcts_find_best_position_for_tile_with_nn(
                &mut plateau,
                &mut deck,
                chosen_tile,
                policy_net,
                value_net,
                num_simulations,
                current_turn,
                total_turns,
            );
            let best_position = game_result.best_position;
            plateau.tiles[best_position] = chosen_tile;
            deck = replace_tile_in_deck(&deck, &chosen_tile);
            current_turn += 1; // Increment turn counter each time a tile is placed

        }

        let game_score = result(&plateau);
        scores.push(game_score);
    }

    let avg_score: f64 = scores.iter().copied().sum::<i32>() as f64 / scores.len() as f64;
    log::info!("Model Evaluation Complete. Avg Score: {:.2}", avg_score);
    // **Stop ping task**
}


/// Checks if the plateau is full
fn is_plateau_full(plateau: &Plateau) -> bool {
    plateau.tiles.iter().all(|tile| *tile != Tile(0, 0, 0))
}

/// Finds the best move using MCTS


/// Simulates num_simulations games and returns the average score

fn simulate_games(plateau: Plateau, deck: Deck) -> i32 {
    let mut simulated_plateau = plateau.clone();
    let simulated_deck = deck.clone();
    let mut legal_moves = get_legal_moves(simulated_plateau.clone());

    // Filter out invalid tiles (0, 0, 0)
    let mut valid_tiles: Vec<Tile> = simulated_deck
        .tiles
        .iter()
        .cloned()
        .filter(|tile| *tile != Tile(0, 0, 0))
        .collect();

    let mut rng = rand::thread_rng(); // Use fast RNG

    while !is_plateau_full(&simulated_plateau) {
        if legal_moves.is_empty() || valid_tiles.is_empty() {
            break;
        }

        // Fast random selection using rand::Rng
        let position_index = rng.gen_range(0..legal_moves.len());
        let position = legal_moves.swap_remove(position_index); // Swap-remove for O(1) removal

        let tile_index = rng.gen_range(0..valid_tiles.len());
        let chosen_tile = valid_tiles.swap_remove(tile_index); // Swap-remove for O(1) removal

        // Place the chosen tile
        simulated_plateau.tiles[position] = chosen_tile;
    }

    result(&simulated_plateau) // Compute and return the result
}



/// Get all legal moves (empty positions) on the plateau
fn get_legal_moves(plateau: Plateau) -> Vec<usize> {
    plateau
        .tiles
        .iter()
        .enumerate()
        .filter_map(|(i, tile)| if *tile == Tile(0, 0, 0) { Some(i) } else { None })
        .collect()
}