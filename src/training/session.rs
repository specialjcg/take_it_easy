//! Self-play training orchestration for the Take It Easy AI.
//!
//! The module provides two entrypoints:
//!
//! - [`train_and_evaluate`], which expects a WebSocket client (front-end) and streams live updates.
//! - [`train_and_evaluate_offline`], which replays games entirely offline for headless or CI workloads.
//!
//! Both variants persist game data to `.pt` files, retrain the policy/value networks, evaluate them
//! periodically, and checkpoint the weights under `model_weights/`.
use crate::data::append_result::append_to_results_file;
use crate::data::load_data::load_game_data_with_arch;
use crate::data::save_data::save_game_data;
use crate::game::create_deck::create_deck;
use crate::game::plateau::create_plateau_empty;
use crate::game::plateau_is_full::is_plateau_full;
use crate::game::remove_tile_from_deck::replace_tile_in_deck;
use crate::game::tile::Tile;
use crate::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use crate::mcts::mcts_result::MCTSResult;
use crate::neural::manager::NNArchitecture;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::neural::training::trainer::train_network_with_game_data;
use crate::scoring::scoring::result;
use crate::training::evaluator::evaluate_model;
use crate::training::websocket::send_websocket_message;
use crate::utils::image::generate_tile_image_names;
use futures_util::StreamExt;
use rand::{rng, Rng};
use std::collections::HashMap;
use std::sync::Arc;
use tch::nn;
use tch::nn::Optimizer;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;

#[derive(Clone, Debug)]
pub struct TrainingOptions {
    pub min_score_high: f64,
    pub min_score_medium: f64,
    pub medium_mix_ratio: f32,
    pub dynamic_sim_boost: usize,
}

impl Default for TrainingOptions {
    fn default() -> Self {
        Self {
            min_score_high: 140.0,
            min_score_medium: 120.0,
            medium_mix_ratio: 0.2,
            dynamic_sim_boost: 50,
        }
    }
}

fn weight_from_score(score: f64, options: &TrainingOptions) -> usize {
    if score >= options.min_score_high + 20.0 {
        3
    } else if score >= options.min_score_high {
        2
    } else if score >= options.min_score_medium {
        1
    } else {
        0
    }
}

fn push_sample(buffer: &mut Vec<MCTSResult>, sample: &MCTSResult, copies: usize) {
    for _ in 0..copies {
        buffer.push(sample.clone());
    }
}

fn adjust_num_simulations(
    base: usize,
    current_turn: usize,
    total_turns: usize,
    options: &TrainingOptions,
) -> usize {
    let early_threshold = total_turns / 3;
    let late_threshold = (total_turns as f32 * 0.75) as usize;

    if current_turn <= early_threshold {
        base + options.dynamic_sim_boost
    } else if current_turn >= late_threshold {
        base + options.dynamic_sim_boost / 2
    } else {
        base
    }
}

/// Lance une session MCTS vs Humain - Version refactorisée avec send_websocket_message
#[allow(clippy::too_many_arguments)]
pub async fn train_and_evaluate(
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
    options: &TrainingOptions,
) {
    let mut total_score = 0;
    let mut games_played = 0;
    let results_file = "results.csv";

    #[allow(clippy::never_loop)]
    while let Ok((stream, _)) = listener.accept().await {
        let ws_stream = accept_async(stream)
            .await
            .expect("Failed to accept WebSocket");
        let (mut write, _) = ws_stream.split();
        let mut scores_by_position: HashMap<usize, Vec<i32>> = HashMap::new();
        let mut scores = Vec::new(); // Stocke les scores
        let evaluation_interval_average = 10;
        let mut training_buffer: Vec<MCTSResult> = Vec::new();

        while games_played < num_games {
            let mut batch_games_played = 0; // Tracks games processed in this evaluation interval
            let max_memory_size = 1000; // Store last 500 games

            for game in 0..evaluation_interval {
                let mut deck = create_deck();
                let mut plateau = create_plateau_empty();
                let mut game_data = Vec::new();
                let mut first_move: Option<(usize, Tile)> = None;
                let total_turns = 19; // The number of moves in the game
                let mut current_turn = 0;

                while !is_plateau_full(&plateau) {
                    let tile_index = rng().random_range(0..deck.tiles.len());
                    let chosen_tile = deck.tiles[tile_index];

                    // ✅ Send preview before placement - REFACTORISÉ
                    let chosen_tile_image = format!(
                        "../image/{}{}{}.png",
                        chosen_tile.0, chosen_tile.1, chosen_tile.2
                    );
                    let payload = serde_json::json!({
                        "next_tile": chosen_tile_image,
                        "plateau_tiles": generate_tile_image_names(&plateau.tiles)
                    });

                    // 🔄 REMPLACEMENT: write.send() → send_websocket_message()
                    if let Err(e) =
                        send_websocket_message(&mut write, payload.to_string(), &listener).await
                    {
                        log::error!("Failed to send tile preview: {}", e);
                        break;
                    }

                    let effective_sims =
                        adjust_num_simulations(num_simulations, current_turn, total_turns, options);
                    let game_result = mcts_find_best_position_for_tile_with_nn(
                        &mut plateau,
                        &mut deck,
                        chosen_tile,
                        policy_net,
                        value_net,
                        effective_sims,
                        current_turn,
                        total_turns,
                        None,
                    );

                    let best_position = game_result.best_position;
                    if first_move.is_none() {
                        first_move = Some((best_position, chosen_tile));
                    }
                    plateau.tiles[best_position] = chosen_tile;
                    deck = replace_tile_in_deck(&deck, &chosen_tile);

                    // ✅ Send score to client - REFACTORISÉ
                    let current_score = result(&plateau);
                    let score_payload = serde_json::json!({
                        "type": "score_update",
                        "current_score": current_score,
                    });

                    // 🔄 REMPLACEMENT: Logique complexe de reconnexion → send_websocket_message()
                    if let Err(e) =
                        send_websocket_message(&mut write, score_payload.to_string(), &listener)
                            .await
                    {
                        log::error!("Failed to send score update: {}", e);
                        break;
                    }

                    game_data.push(game_result); // Store training data

                    // ✅ Send updated plateau state - REFACTORISÉ
                    let payload_after_placement = serde_json::json!({
                        "next_tile": null, // Clear preview
                        "plateau_tiles": generate_tile_image_names(&plateau.tiles) // new updated state
                    });

                    // 🔄 REMPLACEMENT: Logique complexe de reconnexion → send_websocket_message()
                    if let Err(e) = send_websocket_message(
                        &mut write,
                        payload_after_placement.to_string(),
                        &listener,
                    )
                    .await
                    {
                        log::error!("Failed to send plateau update: {}", e);
                        break;
                    }

                    current_turn += 1; // Increment turn counter each time a tile is placed
                }

                let final_score = result(&plateau);

                if let Some((position, _)) = first_move {
                    scores_by_position
                        .entry(position)
                        .or_default()
                        .push(final_score);
                }

                // Historical data (high-quality + medium mix)
                let historical_samples: Vec<MCTSResult> =
                    load_game_data_with_arch("game_data", policy_net.arch)
                        .into_iter()
                        .filter(|r| r.subscore >= options.min_score_medium)
                        .take(200)
                        .collect();

                let (hist_high, hist_medium): (Vec<_>, Vec<_>) = historical_samples
                    .into_iter()
                    .partition(|r| r.subscore >= options.min_score_high);

                let medium_quota = ((hist_high.len() as f32 * options.medium_mix_ratio).round()
                    as usize)
                    .min(hist_medium.len());

                for result in hist_high {
                    let copies = weight_from_score(result.subscore, options);
                    if copies > 0 {
                        push_sample(&mut training_buffer, &result, copies);
                    }
                }

                for result in hist_medium.into_iter().take(medium_quota) {
                    let copies = weight_from_score(result.subscore, options).max(1);
                    push_sample(&mut training_buffer, &result, copies);
                }

                let mut current_high = Vec::new();
                let mut current_medium = Vec::new();
                for result in &game_data {
                    if result.subscore >= options.min_score_high {
                        current_high.push(result);
                    } else if result.subscore >= options.min_score_medium {
                        current_medium.push(result);
                    }
                }

                let current_medium_quota = ((current_high.len() as f32 * options.medium_mix_ratio)
                    .round() as usize)
                    .min(current_medium.len());

                for result in current_high {
                    let copies = weight_from_score(result.subscore, options);
                    if copies > 0 {
                        push_sample(&mut training_buffer, result, copies);
                    }
                }

                for result in current_medium.into_iter().take(current_medium_quota) {
                    let copies = weight_from_score(result.subscore, options).max(1);
                    push_sample(&mut training_buffer, result, copies);
                }

                // Keep only last max_memory_size experiences
                if training_buffer.len() > max_memory_size {
                    let to_remove = training_buffer.len() - max_memory_size;
                    training_buffer.drain(0..to_remove); // Remove oldest data
                }
                scores.push(final_score);

                // Update batch-specific counters
                batch_games_played += 1;
                total_score += final_score;

                if game % evaluation_interval_average == 0 && game != 0 {
                    let moyenne: f64 = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
                    // 🔄 REMPLACEMENT: write.send().await.unwrap() → send_websocket_message()
                    let result_message = format!("GAME_RESULT:{}", moyenne);
                    if let Err(e) =
                        send_websocket_message(&mut write, result_message, &listener).await
                    {
                        log::error!("Failed to send game result: {}", e);
                    }
                }

                // Save current game data for future training
                let filtered_game_data: Vec<MCTSResult> = game_data
                    .into_iter()
                    .filter(|result| result.subscore >= options.min_score_medium)
                    .collect();
                if !filtered_game_data.is_empty() {
                    save_game_data("game_data", filtered_game_data);
                }
            }

            // Update main game counters
            if !training_buffer.is_empty() {
                let batch_size = 16;
                for batch in training_buffer.chunks(batch_size) {
                    train_network_with_game_data(
                        vs_policy,
                        vs_value,
                        batch,
                        0.0,
                        policy_net,
                        value_net,
                        optimizer_policy,
                        optimizer_value,
                    );
                }
                training_buffer.clear();
            }

            games_played += batch_games_played;

            // Append results to the file
            let avg_score = total_score as f64 / games_played as f64;
            if let Err(e) = append_to_results_file(results_file, avg_score) {
                eprintln!("⚠️  Warning: Failed to append results to '{}': {}", results_file, e);
            }

            // Calculate and display averages
            let mut averages: Vec<(usize, f64)> = scores_by_position
                .iter()
                .map(|(position, scores)| {
                    let average_score: f64 =
                        scores.iter().sum::<i32>() as f64 / scores.len() as f64;
                    (*position, average_score)
                })
                .collect();

            averages.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            log::info!("\n--- Average Scores by First Position (Sorted) ---");
            for (position, average_score) in averages {
                log::info!(
                    "Position: {}, Average Score: {:.2}",
                    position,
                    average_score
                );
            }

            // Evaluate model after each interval
            evaluate_model(policy_net, value_net, num_simulations).await;

            // Save model weights avec les bons chemins selon l'architecture
            let arch_dir = match policy_net.arch {
                NNArchitecture::Cnn => "cnn",
                NNArchitecture::Gnn => "gnn",
                NNArchitecture::CnnOnehot => "cnn-onehot",
            };
            let policy_path = format!("model_weights/{}/policy/policy.params", arch_dir);
            let value_path = format!("model_weights/{}/value/value.params", arch_dir);

            if let Err(e) = policy_net.save_model(vs_policy, &policy_path) {
                log::error!("Error saving PolicyNet weights: {:?}", e);
            }
            if let Err(e) = value_net.save_model(vs_value, &value_path) {
                log::error!("Error saving ValueNet weights: {:?}", e);
            }
        }
        break; // Exit after handling one connection
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn train_and_evaluate_offline(
    vs_policy: &nn::VarStore,
    vs_value: &nn::VarStore,
    policy_net: &mut PolicyNet,
    value_net: &mut ValueNet,
    optimizer_policy: &mut Optimizer,
    optimizer_value: &mut Optimizer,
    num_games: usize,
    num_simulations: usize,
    evaluation_interval: usize,
    options: &TrainingOptions,
) {
    let mut total_score = 0;
    let mut games_played = 0;
    let results_file = "results.csv";
    let evaluation_interval_average = 10;
    let max_memory_size = 1000;
    let mut scores_by_position: HashMap<usize, Vec<i32>> = HashMap::new();
    let mut scores = Vec::new();
    let mut training_buffer: Vec<MCTSResult> = Vec::new();

    let eval_interval = evaluation_interval.max(1);

    while games_played < num_games {
        let mut batch_games_played = 0;

        // Charger les données historiques UNE SEULE FOIS par intervalle d'évaluation
        let historical_samples: Vec<MCTSResult> =
            load_game_data_with_arch("game_data", policy_net.arch)
                .into_iter()
                .filter(|r| r.subscore >= options.min_score_medium)
                .filter(|r| {
                    // Pour le GNN, ne garder que les résultats avec plateau/turn renseignés
                    match policy_net.arch {
                        NNArchitecture::Gnn => {
                            r.plateau.is_some()
                                && r.current_turn.is_some()
                                && r.total_turns.is_some()
                        }
                        NNArchitecture::Cnn | NNArchitecture::CnnOnehot => true,
                    }
                })
                .take(200)
                .collect();

        let (hist_high, hist_medium): (Vec<_>, Vec<_>) = historical_samples
            .into_iter()
            .partition(|r| r.subscore >= options.min_score_high);

        let medium_quota = ((hist_high.len() as f32 * options.medium_mix_ratio).round() as usize)
            .min(hist_medium.len());

        for game in 0..eval_interval {
            if games_played + batch_games_played >= num_games {
                break;
            }

            let mut deck = create_deck();
            let mut plateau = create_plateau_empty();
            let mut game_data = Vec::new();
            let mut first_move: Option<(usize, Tile)> = None;
            let total_turns = 19;
            let mut current_turn = 0;

            while !is_plateau_full(&plateau) {
                let tile_index = rng().random_range(0..deck.tiles.len());
                let chosen_tile = deck.tiles[tile_index];

                let effective_sims =
                    adjust_num_simulations(num_simulations, current_turn, total_turns, options);
                let game_result = mcts_find_best_position_for_tile_with_nn(
                    &mut plateau,
                    &mut deck,
                    chosen_tile,
                    policy_net,
                    value_net,
                    effective_sims,
                    current_turn,
                    total_turns,
                    None,
                );

                let best_position = game_result.best_position;
                if first_move.is_none() {
                    first_move = Some((best_position, chosen_tile));
                }
                plateau.tiles[best_position] = chosen_tile;
                deck = replace_tile_in_deck(&deck, &chosen_tile);

                game_data.push(game_result);
                current_turn += 1;
            }

            let final_score = result(&plateau);

            if let Some((position, _)) = first_move {
                scores_by_position
                    .entry(position)
                    .or_default()
                    .push(final_score);
            }

            // Utiliser les données historiques déjà chargées
            for result in &hist_high {
                let copies = weight_from_score(result.subscore, options);
                if copies > 0 {
                    push_sample(&mut training_buffer, result, copies);
                }
            }

            for result in hist_medium.iter().take(medium_quota) {
                let copies = weight_from_score(result.subscore, options).max(1);
                push_sample(&mut training_buffer, result, copies);
            }

            let mut current_high = Vec::new();
            let mut current_medium = Vec::new();
            for result in &game_data {
                if result.subscore >= options.min_score_high {
                    current_high.push(result);
                } else if result.subscore >= options.min_score_medium {
                    current_medium.push(result);
                }
            }

            let current_medium_quota = ((current_high.len() as f32 * options.medium_mix_ratio)
                .round() as usize)
                .min(current_medium.len());

            for result in current_high {
                let copies = weight_from_score(result.subscore, options);
                if copies > 0 {
                    push_sample(&mut training_buffer, result, copies);
                }
            }

            for result in current_medium.into_iter().take(current_medium_quota) {
                let copies = weight_from_score(result.subscore, options).max(1);
                push_sample(&mut training_buffer, result, copies);
            }

            if training_buffer.len() > max_memory_size {
                let to_remove = training_buffer.len() - max_memory_size;
                training_buffer.drain(0..to_remove);
            }

            scores.push(final_score);
            batch_games_played += 1;
            total_score += final_score;

            let progress_game = games_played + batch_games_played;
            if progress_game % 10 == 0 || progress_game == num_games {
                log::info!(
                    "[OfflineTraining] Progress {}/{} (last score {})",
                    progress_game,
                    num_games,
                    final_score
                );
            }

            if game % evaluation_interval_average == 0 && game != 0 {
                let moyenne: f64 = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
                log::info!("[OfflineTraining] GAME_RESULT: {:.2}", moyenne);
            }

            let filtered_game_data: Vec<MCTSResult> = game_data
                .into_iter()
                .filter(|result| result.subscore >= options.min_score_medium)
                .collect();
            if !filtered_game_data.is_empty() {
                save_game_data("game_data", filtered_game_data);
            }
        }

        if !training_buffer.is_empty() {
            let batch_size = 16;
            for batch in training_buffer.chunks(batch_size) {
                train_network_with_game_data(
                    vs_policy,
                    vs_value,
                    batch,
                    0.0,
                    policy_net,
                    value_net,
                    optimizer_policy,
                    optimizer_value,
                );
            }
            training_buffer.clear();
        }

        if batch_games_played == 0 {
            break;
        }

        games_played += batch_games_played;

        let avg_score = total_score as f64 / games_played as f64;
        if let Err(e) = append_to_results_file(results_file, avg_score) {
            eprintln!("⚠️  Warning: Failed to append results to '{}': {}", results_file, e);
        }

        let mut averages: Vec<(usize, f64)> = scores_by_position
            .iter()
            .map(|(position, scores)| {
                let average_score: f64 = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
                (*position, average_score)
            })
            .collect();

        averages.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        log::info!("\n[OfflineTraining] --- Average Scores by First Position (Sorted) ---");
        for (position, average_score) in averages {
            log::info!(
                "[OfflineTraining] Position: {}, Average Score: {:.2}",
                position,
                average_score
            );
        }

        evaluate_model(policy_net, value_net, num_simulations).await;

        // Save model weights avec les bons chemins selon l'architecture
        let arch_dir = match policy_net.arch {
            NNArchitecture::Cnn => "cnn",
            NNArchitecture::Gnn => "gnn",
            NNArchitecture::CnnOnehot => "cnn-onehot",
        };
        let policy_path = format!("model_weights/{}/policy/policy.params", arch_dir);
        let value_path = format!("model_weights/{}/value/value.params", arch_dir);

        if let Err(e) = policy_net.save_model(vs_policy, &policy_path) {
            log::error!("[OfflineTraining] Error saving PolicyNet weights: {:?}", e);
        }
        if let Err(e) = value_net.save_model(vs_value, &value_path) {
            log::error!("[OfflineTraining] Error saving ValueNet weights: {:?}", e);
        }
    }
}
