use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::protocol::Message;
use serde_json;
use rand::{Rng, rng};
use create_plateau_empty::create_plateau_empty;
use create_shuffle_deck::create_shuffle_deck;
use result::result;
use crate::test::{Deck, Plateau, Tile};


mod test;
mod result;
mod remove_tile_from_deck;
mod create_plateau_empty;
mod create_shuffle_deck;

mod policy_value_net;
fn generate_tile_image_names(tiles: &[Tile]) -> Vec<String> {
    tiles.iter().map(|tile| {
        format!("../image/{}{}{}.png", tile.0, tile.1, tile.2)
    }).collect()
}

use clap::Parser;
use tch::{Device, IndexOp, nn, Tensor};
use tch::nn::{Optimizer, OptimizerConfig};
use crate::policy_value_net::{PolicyNet, ValueNet};
use crate::remove_tile_from_deck::replace_tile_in_deck;

#[derive(Parser)]
struct Config {
    /// Number of games to simulate
    #[arg(short = 'g', long, default_value_t = 100)]
    num_games: usize,

    /// Number of simulations per game state in MCTS
    #[arg(short = 's', long, default_value_t = 100)]
    num_simulations: usize,
}


#[tokio::main]
async fn main() {
    let config = Config::parse();
    let model_path = "model_weights";

    // Initialize VarStore
    let vs = nn::VarStore::new(Device::Cpu);
    let input_dim = (3, 5, 5); // Input: 3 channels, 5x5 grid

    // Initialize PolicyNet and ValueNet
    let mut policy_net = PolicyNet::new(&vs, 3, input_dim);
    let mut value_net = ValueNet::new(&vs, 3, input_dim);
    // Load weights if the file exists
    // Load weights for both networks
    if Path::new(model_path).exists() {
        println!("Loading model weights from {}", model_path);
        if let Err(e) = policy_net.load_weights("model_weights/policy") {
            eprintln!("Error loading PolicyNet: {:?}", e);
            println!("Initializing PolicyNet with random weights.");
        }

        if let Err(e) = value_net.load_weights("model_weights/value") {
            eprintln!("Error loading ValueNet: {:?}", e);
            println!("Initializing ValueNet with random weights.");
        }

    } else {
        println!("No pre-trained model found. Initializing new models.");
    }


    // Initialize the optimizer
    let mut optimizer = nn::Adam::default().build(&vs, 1e-3).unwrap();

    // Launch simulations and train the model
    let listener = TcpListener::bind("127.0.0.1:9000")
        .await
        .expect("Unable to start WebSocket server");
    println!("WebSocket server started at ws://127.0.0.1:9000");
    train_and_evaluate(
        &mut policy_net,
        &mut value_net,
        &mut optimizer,
        config.num_games,
        config.num_simulations,
        50, // Evaluate every 50 games
        listener.into(),
    ).await;



}


/// Finds the best move using MCTS with neural network guidance
fn mcts_find_best_position_for_tile_with_nn(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    policy_net: &PolicyNet,
    num_simulations: usize,
) -> Option<usize> {
    let legal_moves = get_legal_moves(plateau.clone());
    if legal_moves.is_empty() {
        return None;
    }

    let board_tensor = convert_plateau_to_tensor(plateau, &chosen_tile, deck);
    let policy_logits = policy_net.forward(&board_tensor, false);
    let policy = policy_logits.softmax(-1, tch::Kind::Float);

    let mut visit_counts: HashMap<usize, usize> = HashMap::new();
    let mut total_scores: HashMap<usize, f64> = HashMap::new();
    let mut ucb_scores: HashMap<usize, f64> = HashMap::new();
    let mut total_visits = 0;

    for &position in &legal_moves {
        visit_counts.insert(position, 0);
        total_scores.insert(position, 0.0);
        ucb_scores.insert(position, f64::NEG_INFINITY);
    }

    for _ in 0..num_simulations {
        // Prioritize legal moves based on policy priors
        let mut moves_with_prior: Vec<_> = legal_moves
            .iter()
            .map(|&pos| (pos, policy.i((0, pos as i64)).double_value(&[])))
            .collect();
        moves_with_prior.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top_k = usize::min(
            legal_moves.len(),
            ((total_visits as f64).sqrt() as usize).max(3)
        );

        // Use a fixed top-k
        let k = usize::min(moves_with_prior.len(), top_k);
        let subset_moves: Vec<usize> = moves_with_prior.iter().take(k).map(|&(pos, _)| pos).collect();


        let depth = subset_moves.len(); // Calculate current subset depth
        for &position in &subset_moves {
            let mut temp_plateau = plateau.clone();
            let mut temp_deck = deck.clone();

            temp_plateau.tiles[position] = chosen_tile;
            temp_deck = replace_tile_in_deck(&temp_deck, &chosen_tile);

            let simulated_score = simulate_games(temp_plateau.clone(), temp_deck.clone());

            let visits = visit_counts.entry(position).or_insert(0);
            *visits += 1;
            total_visits += 1;

            let total_score = total_scores.entry(position).or_insert(0.0);
            *total_score += simulated_score as f64;

            let exploration_param = 1.0 + (depth as f64 / 19.0) + (total_visits as f64).ln() / (10.0 + *visits as f64);
            let prior_prob = policy.i((0, position as i64)).double_value(&[]);
            let average_score = *total_score / (*visits as f64);
            let ucb_score = average_score
                + exploration_param
                * (prior_prob * ((total_visits as f64).ln() / (*visits as f64)).sqrt());

            ucb_scores.insert(position, ucb_score);
        }
    }

    legal_moves
        .into_iter()
        .max_by(|&a, &b| {
            ucb_scores
                .get(&a)
                .unwrap_or(&f64::NEG_INFINITY)
                .partial_cmp(ucb_scores.get(&b).unwrap_or(&f64::NEG_INFINITY))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn train_network_with_game_data(
    game_data: &[(Tensor, Tensor, f64)], // State, policy logits, and value
    discount_factor: f64,               // Discount factor γ
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    optimizer: &mut Optimizer,
) {
    let mut total_policy_loss = Tensor::zeros(&[], tch::kind::FLOAT_CPU);
    let mut total_value_loss = Tensor::zeros(&[], tch::kind::FLOAT_CPU);

    let mut future_value  = 0.0; // Initialize for the final state
    let mut current_iteration = 0;
    // Process game data in reverse for TD updates
    for (state, target_policy, reward) in game_data.iter().rev() {
        let pred_policy = policy_net.forward(state, true);
        let pred_value = value_net.forward(state, true);
        current_iteration += 1;
        // Normalize reward for stability
        let mean_reward = game_data.iter().map(|(_, _, r)| *r).sum::<f64>() / game_data.len() as f64;
        let std_dev_reward = (game_data.iter().map(|(_, _, r)| (*r - mean_reward).powi(2)).sum::<f64>() / game_data.len() as f64).sqrt();
        let normalized_reward = (reward - mean_reward) / std_dev_reward;
        // Compute TD target
        let td_target = normalized_reward
            + discount_factor
            * if future_value > pred_value.double_value(&[]) {
            future_value
        } else {
            pred_value.double_value(&[])
        };
        let target_tensor = Tensor::from(td_target);

        // Compute policy loss
        let policy_loss = -(target_policy * pred_policy.log()).sum(tch::Kind::Float);
        total_policy_loss += policy_loss;

        // Compute value loss (TD learning)
        let pred_value = pred_value.view([]); // Ensure scalar
        let value_loss = (target_tensor - pred_value).pow(&Tensor::from(2.0));
        total_value_loss += value_loss;

        // Update future value
        future_value = td_target;
    }

    let policy_loss_weight = 1.0;
    let value_loss_weight = if current_iteration < 100 { 0.5 } else { 1.0 };

    let total_loss = policy_loss_weight * total_policy_loss + value_loss_weight * total_value_loss;

    // Optimize
    optimizer.backward_step(&total_loss);
}



fn convert_plateau_to_tensor(plateau: &Plateau, tile: &Tile, deck: &Deck) -> Tensor {
    let mut features = vec![0.0; 3 * 5 * 5];

    // Encode the plateau tiles
    for (i, t) in plateau.tiles.iter().enumerate() {
        let row = i / 5;
        let col = i % 5;
        features[row * 5 + col] = t.0 as f32 / 10.0;       // Normalize first feature
        features[5 * 5 + row * 5 + col] = t.1 as f32 / 10.0; // Normalize second feature
        features[2 * 5 * 5 + row * 5 + col] = t.2 as f32 / 10.0; // Normalize third feature
    }

    // Encode the currently chosen tile in the center of the grid
    let center_row = 2;
    let center_col = 2;
    features[center_row * 5 + center_col] += tile.0 as f32 / 10.0;       // Add tile's first feature
    features[5 * 5 + center_row * 5 + center_col] += tile.1 as f32 / 10.0; // Add tile's second feature
    features[2 * 5 * 5 + center_row * 5 + center_col] += tile.2 as f32 / 10.0; // Add tile's third feature

    // Include deck information as additional features
    for (i, deck_tile) in deck.tiles.iter().enumerate() {
        let normalized_feature = (deck_tile.0 as f32 + deck_tile.1 as f32 + deck_tile.2 as f32) / (3.0 * 10.0);
        let row = i / 5;
        let col = i % 5;
        features[row * 5 + col] += normalized_feature; // Accumulate in the first channel
    }

    // Add pattern-based features
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

    // Add pattern-based features
    for (positions, _, feature_fn) in patterns {
        for &pos in positions {
            if let Some(t) = plateau.tiles.get(pos) {
                let row = pos / 5;
                let col = pos % 5;
                let contribution = feature_fn(t) as f32 / 10.0 * 0.1; // Scale the pattern contribution
                features[row * 5 + col] += contribution*1.2;
            }
        }
    }

    // Create a tensor with the specified shape [1, 3, 5, 5]
    Tensor::of_slice(&features).view([1, 3, 5, 5])
}
fn load_game_data(file_path: &str) -> Vec<(Tensor, Tensor, f64)> {
    let file = File::open(file_path).expect("Unable to open file");
    let reader = BufReader::new(file);

    let mut game_data = Vec::new();
    for line in reader.lines() {
        if let Ok(line) = line {


                game_data.push(deserialize_game_data(&line));

        }
    }
    game_data
}

fn deserialize_game_data(line: &str) -> (Tensor, Tensor, f64) {
    let parts: Vec<&str> = line.split(',').collect();

    if parts.len() != 3 {
        panic!("Invalid data format. Expected 3 parts (state, policy, value).");
    }

    // Deserialize the state tensor
    let state_values: Vec<f32> = parts[0]
        .split_whitespace()
        .map(|v| v.parse::<f32>().expect("Invalid float in state tensor"))
        .collect();
    let state_tensor = Tensor::of_slice(&state_values).view([1, 3, 5, 5]); // Adjust the dimensions as needed

    // Deserialize the policy tensor
    let policy_values: Vec<f32> = parts[1]
        .split_whitespace()
        .map(|v| v.parse::<f32>().expect("Invalid float in policy tensor"))
        .collect();
    let policy_tensor = Tensor::of_slice(&policy_values).view([1, 19]); // Adjust dimensions for policy logits

    // Deserialize the value
    let value: f64 = parts[2]
        .parse::<f64>()
        .unwrap_or_else(|_| panic!("Invalid float in value: {}", parts[2]));

    (state_tensor, policy_tensor, value)
}
fn save_game_data(file_path: &str, game_data: Vec<(Tensor, Tensor, f64)>) {
    use std::fs::OpenOptions;
    use std::io::{Write, BufWriter};

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)
        .expect("Unable to open file");
    let mut writer = BufWriter::new(file);

    for (state, policy, value) in game_data {
        let state_str = serialize_tensor(state);
        let policy_str = serialize_tensor(policy);
        writeln!(writer, "{},{},{}", state_str, policy_str, value)
            .expect("Unable to write data");
    }
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
    let data: Vec<f32> =tensor_to_vec(&tensor) ;// Converts the slice to a Vec<f32>
        data.iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}


async fn train_and_evaluate(
    policy_net: &mut PolicyNet,
    value_net: &mut ValueNet<'_>,
    optimizer: &mut Optimizer,
    num_games: usize,
    num_simulations: usize,
    evaluation_interval: usize,
    listener: Arc<TcpListener>,
) {
    let mut total_score = 0;
    let mut games_played = 0;

    while let Ok((stream, _)) = listener.accept().await {
        let ws_stream = accept_async(stream).await.expect("Failed to accept WebSocket");
        let (mut write, _) = ws_stream.split();
        let mut scores_by_position: HashMap<usize, Vec<i32>> = HashMap::new();
        let mut scores = Vec::new(); // Stocke les scores
        let evaluation_interval_average = 10;

        while games_played < num_games {
            println!(
                "Starting training iteration {}/{}...",
                games_played + 1,
                num_games
            );

            for game in 0..evaluation_interval {
                let mut deck = create_shuffle_deck();
                let mut plateau = create_plateau_empty();
                let mut game_data = Vec::new();
                let mut first_move: Option<(usize, Tile)> = None;

                while !is_plateau_full(&plateau) {
                    let tile_index = rng().random_range(0..deck.tiles.len());
                    let chosen_tile = deck.tiles[tile_index];

                    if let Some(best_position) = mcts_find_best_position_for_tile_with_nn(
                        &mut plateau,
                        &mut deck,
                        chosen_tile,
                        policy_net,
                        num_simulations,
                    ) {
                        if first_move.is_none() {
                            first_move = Some((best_position, chosen_tile));
                        }
                        plateau.tiles[best_position] = chosen_tile;
                        deck = replace_tile_in_deck(&deck, &chosen_tile);

                        // Save state, policy, and value for training
                        let board_tensor =
                            convert_plateau_to_tensor(&plateau, &chosen_tile, &deck);
                        let policy_logits = policy_net.forward(&board_tensor, false);
                        let value = value_net.forward(&board_tensor, false);
                        let reward = value.double_value(&[]); // Convert Tensor to f64
                        game_data.push((board_tensor, policy_logits, reward)); // Use f64 for the third element
                        let image_names = generate_tile_image_names(&plateau.tiles);
                        let serialized = serde_json::to_string(&image_names).unwrap();
                        write.send(Message::Text(serialized)).await.unwrap();
                    }
                }
                let final_score = result(&plateau);
                total_score += final_score;
                if let Some((position, _)) = first_move {
                    scores_by_position
                        .entry(position)
                        .or_insert_with(Vec::new)
                        .push(final_score);
                }
                let mut batch_game_data = Vec::new();
                let historical_game_data = load_game_data("game_data.csv");

                // Add historical data to the training batch
                for (state, policy, value) in &historical_game_data {
                    batch_game_data.push((state.shallow_clone(), policy.shallow_clone(), value.clone()));
                }
                for (state, policy, value) in &game_data {
                    batch_game_data.push((state.shallow_clone(), policy.shallow_clone(), value.clone()));
                }

                let batch_size = 10;
                if batch_game_data.len() > batch_size {
                    train_network_with_game_data(&batch_game_data, final_score.into(), policy_net, value_net, optimizer);
                    batch_game_data.clear();
                }

                println!("Game {} finished with score: {}", game + 1, final_score);
                scores.push(final_score);
                if game % evaluation_interval_average == 0 {
                    let moyenne: f64 = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
                    println!("Partie {} - Score moyen: {:.2}", game, moyenne);
                    write.send(Message::Text(format!("GAME_RESULT:{}", moyenne))).await.unwrap();

                }
                save_game_data("game_data.csv", game_data);
            }


            games_played += evaluation_interval;

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

            println!("\n--- Average Scores by First Position (Sorted) ---");
            for (position, average_score) in averages {
                println!("Position: {}, Average Score: {:.2}", position, average_score);
            }

            // Evaluate model after each interval
            evaluate_model(policy_net,  num_simulations).await;

            println!(
                "Games Played: {}, Total Score: {}, Avg Score: {:.2}",
                games_played,
                total_score,
                total_score as f32 / games_played as f32
            );
            let model_path = "model_weights";
            // Save model weights
            println!("Saving models to {}", model_path);
            println!("Saving model weights...");
            if let Err(e) = policy_net.save_weights("model_weights/policy") {
                eprintln!("Error saving PolicyNet weights: {:?}", e);
            }
            if let Err(e) = value_net.save_weights("model_weights/value") {
                eprintln!("Error saving ValueNet weights: {:?}", e);
            }

        }

        break; // Exit after handling one connection
    }
}




async fn evaluate_model(
    policy_net: &PolicyNet,
    num_simulations: usize,
) {
    println!("Evaluating model...");
    let mut scores = Vec::new();

    for _ in 0..10 {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();

        while !is_plateau_full(&plateau) {
            let tile_index = rng().random_range(0..deck.tiles.len());
            let chosen_tile = deck.tiles[tile_index];

            if let Some(best_position) = mcts_find_best_position_for_tile_with_nn(
                &mut plateau,
                &mut deck,
                chosen_tile,
                policy_net,
                num_simulations,
            ) {
                plateau.tiles[best_position] = chosen_tile;
                deck = replace_tile_in_deck(&deck, &chosen_tile);
            }
        }

        let game_score = result(&plateau);
        scores.push(game_score);
    }

    let avg_score: f64 = scores.iter().copied().sum::<i32>() as f64 / scores.len() as f64;
    println!("Model Evaluation Complete. Avg Score: {:.2}", avg_score);
}






/// Checks if the plateau is full
fn is_plateau_full(plateau: &Plateau) -> bool {
    plateau.tiles.iter().all(|tile| *tile != Tile(0, 0, 0))
}

/// Finds the best move using MCTS


/// Simulates `num_simulations` games and returns the average score
fn simulate_games(plateau: Plateau, deck: Deck) -> i32 {
    let mut simulated_plateau = plateau.clone();
    let mut simulated_deck = deck.clone();
    let mut legal_moves = get_legal_moves(simulated_plateau.clone());

    // Filter out invalid tiles (0, 0, 0)
    let mut valid_tiles: Vec<Tile> = simulated_deck
        .tiles
        .iter()
        .cloned()
        .filter(|tile| *tile != Tile(0, 0, 0))
        .collect();

    if legal_moves.len() <= 2 {
        let mut stack = Vec::new();
        let mut best_score = i32::MIN;

        // Push initial states to stack
        stack.push((simulated_plateau.clone(), valid_tiles.clone(), 0));

        while let Some((current_plateau, remaining_tiles, depth)) = stack.pop() {
            if depth >= legal_moves.len() {
                // Evaluate the final score of this state
                let score = result(&current_plateau);
                if score > best_score {
                    best_score = score;
                }
                continue;
            }

            for &position in &legal_moves {
                for tile in &remaining_tiles {
                    let mut new_plateau = current_plateau.clone();
                    let mut new_tiles = remaining_tiles.clone();

                    // Place the tile
                    new_plateau.tiles[position] = *tile;

                    // Remove the tile from the list
                    new_tiles.retain(|t| t != tile);

                    // Push the new state to the stack
                    stack.push((new_plateau, new_tiles, depth + 1));
                }
            }
        }

        return best_score; // Return the best score found
    }

    // Regular simulation for more tiles
    let mut rng = rng();
    while !is_plateau_full(&simulated_plateau) {
        if legal_moves.is_empty() {
            break;
        }

        let position = legal_moves[rng.random_range(0..legal_moves.len())];
        let tile_index = rng.random_range(0..valid_tiles.len());
        let chosen_tile = valid_tiles[tile_index];

        // Remove the chosen position from legal moves
        if let Some(index) = legal_moves.iter().position(|&x| x == position) {
            legal_moves.swap_remove(index);
        }

        // Place the chosen tile
        simulated_plateau.tiles[position] = chosen_tile;

        // Remove the chosen tile from the list of valid tiles
        valid_tiles.retain(|t| t != &chosen_tile);
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

