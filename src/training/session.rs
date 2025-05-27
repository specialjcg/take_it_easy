use crate::data::append_result::append_to_results_file;
use crate::data::load_data::load_game_data;
use crate::data::save_data::save_game_data;
use crate::game::create_deck::create_deck;
use crate::game::plateau::create_plateau_empty;
use crate::game::plateau_is_full::is_plateau_full;
use crate::game::remove_tile_from_deck::replace_tile_in_deck;
use crate::game::tile::Tile;
use crate::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use crate::mcts::mcts_result::MCTSResult;
use crate::mcts_vs_human::play_mcts_vs_human;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::neural::training::trainer::train_network_with_game_data;
use crate::scoring::scoring::result;
use crate::training::evaluator::evaluate_model;
use crate::training::websocket::reconnect_websocket;
use crate::utils::image::generate_tile_image_names;
use crate::Config;
use futures_util::{SinkExt, StreamExt};
use rand::{rng, Rng};
use std::collections::HashMap;
use std::sync::Arc;
use tch::nn;
use tch::nn::Optimizer;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
/// Lance une session MCTS vs Humain

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
) {
    let mut total_score = 0;
    let mut games_played = 0;
    let results_file = "results.csv";

    while let Ok((stream, _)) = listener.accept().await {
        let ws_stream = accept_async(stream)
            .await
            .expect("Failed to accept WebSocket");
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
            log::info!(
                "\n🚀 Starting Batch {}",
                games_played / evaluation_interval + 1
            );

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
                    // ✅ **Send preview before placement**
                    // ✅ **INSERT YOUR NEW CODE HERE**
                    let chosen_tile_image = format!(
                        "../image/{}{}{}.png",
                        chosen_tile.0, chosen_tile.1, chosen_tile.2
                    );
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
                    .into_iter()
                    .filter(|r| r.subscore > 100.0) // Only select high-score games
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
                    log::info!(
                        "📊 [Batch {}] Avg Score: {:.2} | Games Played: {}",
                        games_played / evaluation_interval,
                        moyenne,
                        games_played
                    );
                    log::info!("batch {} - Score moyen: {:.2}", game, moyenne);
                    write
                        .send(Message::Text(format!("GAME_RESULT:{}", moyenne)))
                        .await
                        .unwrap();
                }

                // Save current game data for future training
                save_game_data("game_data", game_data);
            }

            // Update main game counters
            games_played += batch_games_played;

            // Append results to the file
            let avg_score = total_score as f64 / games_played as f64;
            append_to_results_file(results_file, avg_score);

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
            if let Err(e) = policy_net.save_model(vs_policy, "model_weights/policy/policy.params") {
                log::error!("Error saving PolicyNet weights: {:?}", e);
            }
            if let Err(e) = value_net.save_model(vs_value, "model_weights/value/value.params") {
                log::error!("Error saving ValueNet weights: {:?}", e);
            }
        }
        break; // Exit after handling one connection
    }
}
