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

use crate::policy_value_net::{PolicyNet, ValueNet};
use crate::remove_tile_from_deck::{remove_tile_from_deck, replace_tile_in_deck};
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

#[derive(Parser)]
struct Config {
    /// Number of games to simulate
    #[arg(short = 'g', long, default_value_t = 500)]
    num_games: usize,

    /// Number of simulations per game state in MCTS
    #[arg(short = 's', long, default_value_t = 3000)]
    num_simulations: usize,
}


#[tokio::main]
async fn main() {
    let config = Config::parse();
    let model_path = "model_weights";

    // Initialize VarStore
    let mut vs = nn::VarStore::new(Device::Cpu);
    let input_dim = (5, 47,1); // (Channels, Height, Width)

    // Initialize PolicyNet and ValueNet
    let mut policy_net = PolicyNet::new(&vs,  input_dim);
    let mut value_net = ValueNet::new(&mut vs,  input_dim);
    // Load weights if the file exists
    // Load weights for both networks
    if Path::new(model_path).exists() {
        println!("Loading model weights from {}", model_path);
        if let Err(e) = policy_net.load_model(&mut vs, "model_weights/policy/policy.params" ) {
            eprintln!("Error loading PolicyNet: {:?}", e);
            println!("Initializing PolicyNet with random weights.");
        }

        if let Err(e) = value_net.load_model(&mut vs, "model_weights/value/value.params") {
            eprintln!("Error loading ValueNet: {:?}", e);
            println!("Initializing ValueNet with random weights.");
        }
    } else {
        println!("No pre-trained model found. Initializing new models.");
    }

    let mut optimizer = nn::Adam {
        wd: 5e-4, // Weight decay
        ..Default::default() // Other defaults
    }
        .build(&vs, 1e-4) // Learning rate
        .unwrap();


    // Launch simulations and train the model
    let listener = TcpListener::bind("127.0.0.1:9000")
        .await
        .expect("Unable to start WebSocket server");
    println!("WebSocket server started at ws://127.0.0.1:9000");
    train_and_evaluate(
        &vs,
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
        let pred_value = pred_value.clamp(-2.0, 2.0);

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
            ((total_visits as f64).sqrt() as usize).max(3),
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

            let total_simulated_score: f64 = (0..rollout_count)
                .map(|_| simulate_games(temp_plateau.clone(), temp_deck.clone()) as f64)
                .sum();
            let simulated_score = total_simulated_score / rollout_count as f64;

            let visits = visit_counts.entry(position).or_insert(0);
            *visits += 1;
            total_visits += 1;

            let total_score = total_scores.entry(position).or_insert(0.0);
            *total_score += simulated_score as f64;

            let exploration_param = c_puct * (total_visits as f64).ln() / (1.0 + *visits as f64);
            let prior_prob = policy.i((0, position as i64)).double_value(&[]);
            let average_score = *total_score / (*visits as f64);
            let ucb_score = average_score
                + exploration_param * (prior_prob.sqrt()) // Use sqrt instead of multiplication
                + 1.2 * value_estimate; // Increase weight of value network estimates

            // **Improved Neural-Guided UCB Calculation**

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

    let best_position_score = total_scores.get(&best_position).cloned().unwrap_or(0.0);

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

    MCTSResult {
        best_position,
        board_tensor,
        subscore: final_score as f64, // Store real final score, not UCB score
    }
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



fn normalize_input(tensor: &Tensor, global_mean: &Tensor, global_std: &Tensor) -> Tensor {
    (tensor - global_mean) / (global_std + 1e-8)
}


fn train_network_with_game_data(
    vs: &nn::VarStore,
    game_data: &[MCTSResult],
    discount_factor: f64,
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    optimizer: &mut Optimizer,
) {
    let entropy_weight = 0.05;
    let gamma = 0.99;
    let epsilon = 1e-8;

    let min_reward = game_data.iter().map(|r| r.subscore).fold(f64::INFINITY, f64::min);
    let max_reward = game_data.iter().map(|r| r.subscore).fold(f64::NEG_INFINITY, f64::max);
    let range = (max_reward - min_reward).max(epsilon);

    let normalize_reward = |reward: f64| -> f64 {
        ((reward - min_reward) / range) * 2.0 - 1.0
    };

    let (global_mean, global_std) = compute_global_stats(game_data);

    let mut future_value = Tensor::zeros(&[], (tch::Kind::Float, tch::Device::Cpu));
    let mut total_policy_loss = Tensor::zeros(&[], tch::kind::FLOAT_CPU);
    let mut total_value_loss = Tensor::zeros(&[], tch::kind::FLOAT_CPU);
    let mut total_entropy_loss = Tensor::zeros(&[], tch::kind::FLOAT_CPU);

    for result in game_data.iter().rev() {
        let state = normalize_input(&result.board_tensor, &global_mean, &global_std);

        // Forward pass
        let pred_policy = policy_net.forward(&state, true).clamp_min(1e-7);
        let pred_value = value_net.forward(&state, true).clamp(-10.0, 10.0);

        let normalized_reward_tensor = Tensor::from(normalize_reward(result.subscore));
        let discounted_reward = normalized_reward_tensor + gamma * future_value.shallow_clone().detach();
        future_value = discounted_reward.shallow_clone();

        // Policy Loss
        let best_position = result.best_position as i64;
        let target_policy = Tensor::zeros(&[1, pred_policy.size()[1]], tch::kind::FLOAT_CPU);
        target_policy.i((0, best_position)).fill_(1.0);
        let log_policy = pred_policy.log();
        let policy_loss = -(target_policy * log_policy.shallow_clone()).sum(tch::Kind::Float);
        total_policy_loss += policy_loss;

        // Entropy Loss
        let entropy_loss = -(pred_policy * log_policy).sum(tch::Kind::Float);
        total_entropy_loss += entropy_loss;

        // Value Loss
        let td_target = (discounted_reward.shallow_clone() + discount_factor * future_value.shallow_clone())
            .clamp(-10.0, 10.0);
        let value_loss = (td_target - pred_value).pow(&Tensor::from(2.0)).mean(tch::Kind::Float);
        total_value_loss += value_loss;
    }

    // Total Loss
    let total_loss: Tensor = total_policy_loss + total_value_loss + entropy_weight * total_entropy_loss;
    total_loss.backward();

    optimizer.clip_grad_norm(1.0);
    optimizer.step();
    optimizer.zero_grad();
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




// fn convert_plateau_to_tensor(plateau: &Plateau, tile: &Tile, deck: &Deck) -> Tensor {
//     let mut features = vec![0.0; 3 * 47]; // 3 channels, 47 positions (19 plateau + 1 chosen tile + 27 deck)
//
//     // Encode plateau tiles
//     for (i, t) in plateau.tiles.iter().enumerate() {
//         if i < 19 {
//             features[i] = (t.0 as f32 / 10.0).clamp(0.0, 1.0);
//             features[47 + i] = (t.1 as f32 / 10.0).clamp(0.0, 1.0);
//             features[2 * 47 + i] = (t.2 as f32 / 10.0).clamp(0.0, 1.0);
//         }
//     }
//
//     // Encode chosen tile at index 19
//     let tile_index = 19;
//     features[tile_index] = (tile.0 as f32 / 10.0).clamp(0.0, 1.0);
//     features[47 + tile_index] = (tile.1 as f32 / 10.0).clamp(0.0, 1.0);
//     features[2 * 47 + tile_index] = (tile.2 as f32 / 10.0).clamp(0.0, 1.0);
//
//     // Encode deck tiles (remaining 27)
//     for (i, t) in deck.tiles.iter().enumerate() {
//         let deck_index = 20 + i; // Deck starts at index 20
//         if deck_index < 47 {
//             features[deck_index] = (t.0 as f32 / 10.0).clamp(0.0, 1.0);
//             features[47 + deck_index] = (t.1 as f32 / 10.0).clamp(0.0, 1.0);
//             features[2 * 47 + deck_index] = (t.2 as f32 / 10.0).clamp(0.0, 1.0);
//         }
//     }
//
//     // Convert to tensor with correct shape [1, 3, 47, 1]
//     let tensor = Tensor::of_slice(&features).view([1, 3, 47, 1]);
//
//
//     tensor
// }





fn load_game_data<'a>(file_path: &'a str) -> impl Iterator<Item = MCTSResult> + 'a {
    let file = File::open(file_path).expect("Unable to open file");
    let reader = BufReader::new(file);

    reader.lines().filter_map(move |line| {
        match line {
            Ok(line_content) => deserialize_game_data(&line_content),
            Err(e) => {
                eprintln!("Error reading line from file '{}': {}", file_path, e);
                None
            }
        }
    })
}




fn deserialize_game_data(line: &str) -> Option<MCTSResult> {
    let parts: Vec<&str> = line.split(',').collect();

    if parts.len() != 3 {
        eprintln!("Invalid data format: '{}'", line);
        return None;
    }

    // Parse tensor
    let state_values: Vec<f32> = parts[0]
        .split_whitespace()
        .map(|v| v.parse::<f32>())
        .collect::<Result<Vec<f32>, _>>()
        .unwrap_or_else(|_| {
            eprintln!("Failed to parse state tensor in line '{}'", line);
            vec![]
        });

    if state_values.len() != 5 * 47 {
        eprintln!("ERROR: Parsed tensor has incorrect size {} (expected 235). Data: '{}'", state_values.len(), parts[0]);
        return None;
    }

    let state_tensor = Tensor::of_slice(&state_values).view([1, 5, 47, 1]);

    // Parse subscore
    let subscore = parts[1].parse::<f64>().unwrap_or_else(|_| {
        eprintln!("Failed to parse subscore in line '{}'", line);
        0.0
    });

    // Parse best position
    let best_position = parts[2].parse::<usize>().unwrap_or_else(|_| {
        eprintln!("Failed to parse best_position in line '{}'", line);
        0
    });

    Some(MCTSResult {
        board_tensor: state_tensor,
        subscore,
        best_position,
    })
}




fn save_game_data(file_path: &str, game_data: Vec<MCTSResult>) {
    use std::fs::OpenOptions;
    use std::io::{BufWriter, Write};

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)
        .expect("Unable to open file");
    let mut writer = BufWriter::new(file);

    for result in game_data {
        let state_str = serialize_tensor(result.board_tensor);
        let best_position_str=result.best_position;


        writeln!(
            writer,
            "{},{},{}",
            state_str, result.subscore, best_position_str
        )
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
    let data: Vec<f32> = tensor_to_vec(&tensor); // Converts the slice to a Vec<f32>
    data.iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

struct MCTSResult {
    board_tensor: Tensor,
    best_position: usize,
    subscore: f64,
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
            println!("Re-establishing WebSocket connection...");
            let ws_stream = accept_async(stream).await.expect("Failed to accept WebSocket");
            let (write, _) = ws_stream.split();
            Some(write)
        }
        Err(e) => {
            eprintln!("Error while reconnecting WebSocket: {:?}", e);
            None
        }
    }
}
async fn train_and_evaluate(
    vs: &nn::VarStore,
    policy_net: &mut PolicyNet,
    value_net: &mut ValueNet,
    optimizer: &mut Optimizer,
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

            println!(
                "Starting training iteration {}/{}...",
                games_played + 1,
                num_games
            );

            let mut batch_games_played = 0; // Tracks games processed in this evaluation interval

            let max_memory_size = 1000; // Store last 500 games

            for game in 0..evaluation_interval {
                let mut deck = create_shuffle_deck();
                let mut plateau = create_plateau_empty();
                let mut game_data = Vec::new();
                let total_turns = 19;
                let mut current_turn = 0;

                while !is_plateau_full(&plateau) {
                    let tile_index = rand::thread_rng().gen_range(0..deck.tiles.len());
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
                    game_data.push(game_result);

                    current_turn += 1;
                }

                let final_score = result(&plateau);

                // Discount rewards in reverse to emphasize good actions
                let gamma = 0.98;
                let mut discounted_reward = final_score as f64;
                let mut improved_game_data = Vec::new();

                for result in game_data.iter().rev() {
                    improved_game_data.push(MCTSResult {
                        best_position: result.best_position,
                        board_tensor: result.board_tensor.shallow_clone(),
                        subscore: discounted_reward,
                    });

                    discounted_reward *= gamma;
                }

                improved_game_data.reverse();
                save_game_data("game_data.csv", improved_game_data);

                println!("Game {} finished with score: {}", game + 1, final_score);

                // ✅ Dynamic selection of top historical game data
                let historical_game_data: Vec<MCTSResult> = load_game_data("game_data.csv").collect();

                let top_percentile_threshold = compute_top_percentile_threshold(&historical_game_data, 0.90);

                println!("Computed threshold for top 10%: {}", top_percentile_threshold);

                let prioritized_data: Vec<MCTSResult> = historical_game_data
                    .into_iter()
                    .filter(|r| r.subscore >= top_percentile_threshold)
                    .take(100)
                    .collect();

                // Merge with current improved game data
                let mut batch_game_data = Vec::new();
                batch_game_data.extend(prioritized_data);
                batch_game_data.extend(improved_game_data);

                // Train on batch_game_data
                let batch_size = 10;
                for batch in batch_game_data.chunks(batch_size) {
                    train_network_with_game_data(
                        &vs,
                        batch,
                        gamma,
                        policy_net,
                        value_net,
                        optimizer,
                    );
                }

                scores_by_position
                    .entry(best_position)
                    .or_insert_with(Vec::new)
                    .push(final_score);
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

            println!("\n--- Average Scores by First Position (Sorted) ---");
            for (position, average_score) in averages {
                println!("Position: {}, Average Score: {:.2}", position, average_score);
            }

            // Evaluate model after each interval
            evaluate_model(policy_net, value_net,num_simulations).await;

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
            if let Err(e) = policy_net.save_model(vs,"model_weights/policy/policy.params") {
                eprintln!("Error saving PolicyNet weights: {:?}", e);
            }
            if let Err(e) = value_net.save_model(vs,"model_weights/value/value.params") {
                eprintln!("Error saving ValueNet weights: {:?}", e);
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
    println!("Evaluating model...");
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
    println!("Model Evaluation Complete. Avg Score: {:.2}", avg_score);
    // **Stop ping task**
}


/// Checks if the plateau is full
fn is_plateau_full(plateau: &Plateau) -> bool {
    plateau.tiles.iter().all(|tile| *tile != Tile(0, 0, 0))
}

/// Finds the best move using MCTS


/// Simulates `num_simulations` games and returns the average score

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

