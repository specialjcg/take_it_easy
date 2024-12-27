use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::protocol::Message;
use serde_json;
use rand::{Rng, rng, SeedableRng, thread_rng};
use rand::rngs::StdRng;
use create_plateau_empty::create_plateau_empty;
use create_shuffle_deck::create_shuffle_deck;
use remove_tile_from_deck::remove_tile_from_deck;
use result::result;
use crate::place_tile::placer_tile;
use crate::test::{Deck, Plateau, Tile};


mod test;
mod result;
mod place_tile;
mod choisir_et_placer;
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
use serde_json::json;
use tch::{Device, IndexOp, nn, Tensor};
use tch::nn::{Optimizer, OptimizerConfig};
use crate::policy_value_net::PolicyValueNet;
use crate::remove_tile_from_deck::replace_tile_in_deck;

#[derive(Parser)]
struct Config {
    /// Number of games to simulate
    #[arg(short = 'g', long, default_value_t = 200)]
    num_games: usize,

    /// Number of simulations per game state in MCTS
    #[arg(short = 's', long, default_value_t = 2000)]
    num_simulations: usize,
}


#[tokio::main]
async fn main() {
    let config = Config::parse();
    let model_path = "model_weights";

    // Initialize VarStore
    let mut vs = nn::VarStore::new(Device::Cpu);
    let input_dim = (3, 5, 5); // Input: 3 channels, 5x5 grid
    let mut policy_value_net = PolicyValueNet::new_with_var_store(&vs, 10, input_dim);
    // Load weights if the file exists
    if Path::new(model_path).exists() {
        println!("Loading model from {}", model_path);
        if let Err(e) = policy_value_net.load_weights(model_path) {
            eprintln!("Error while loading: {:?}", e);
            eprintln!("A fresh model will be used.");
        }
    } else {
        println!("No pre-trained model found. Creating a new model.");
    }


    // Initialize the optimizer
    let mut optimizer = nn::Adam::default().build(&vs, 1e-3).unwrap();

    // Launch simulations and train the model
    let listener = TcpListener::bind("127.0.0.1:9000")
        .await
        .expect("Unable to start WebSocket server");
    println!("WebSocket server started at ws://127.0.0.1:9000");
    train_and_evaluate(
        &mut policy_value_net,
        &mut optimizer,
        config.num_games,
        config.num_simulations,
        50, // Evaluate every 50 games
        listener.into(),
    ).await;

    // run_simulations_with_training(
    //     config.num_games,
    //     config.num_simulations,
    //     listener,
    //     &policy_value_net,
    //     &mut optimizer,
    // )
    //     .await;

    // Save the model
    println!("Saving model to {}", model_path);
    if let Err(e) = policy_value_net.save_weights(&model_path) {
        eprintln!("Error while saving: {:?}", e);
    } else {
        println!("Model saved successfully.");
    }
}


/// Finds the best move using MCTS with neural network guidance
fn mcts_find_best_position_for_tile_with_nn(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    policy_value_net: &PolicyValueNet,
    num_simulations: usize,
) -> Option<usize> {
    let legal_moves = get_legal_moves(plateau.clone());
    if legal_moves.is_empty() {
        return None;
    }

    let board_tensor = convert_plateau_to_tensor(plateau, &chosen_tile, deck);
    let (policy_logits, _value) = policy_value_net.forward(&board_tensor);
    let policy = policy_logits.softmax(-1, tch::Kind::Float);

    let mut visit_counts: HashMap<usize, usize> = HashMap::new();
    let mut total_scores: HashMap<usize, f64> = HashMap::new();
    let mut ucb_scores: HashMap<usize, f64> = HashMap::new();
    let mut total_visits = 0;

    // Initialize visit counts, total scores, and UCB scores for each move
    for &position in &legal_moves {
        visit_counts.insert(position, 0);
        total_scores.insert(position, 0.0);
        ucb_scores.insert(position, f64::NEG_INFINITY);
    }

    for _ in 0..num_simulations {
        for &position in &legal_moves {
            let mut temp_plateau = plateau.clone();
            let mut temp_deck = deck.clone();

            // Simulate placing the tile at this position
            temp_plateau.tiles[position] = chosen_tile;
            temp_deck = replace_tile_in_deck(&temp_deck, &chosen_tile);

            // Simulate the rest of the game to get the final score
            let simulated_score = simulate_games(temp_plateau.clone(), temp_deck.clone(), 1);

            // Update visit count and total score
            let visits = visit_counts.entry(position).or_insert(0);
            *visits += 1;
            total_visits += 1;

            let total_score = total_scores.entry(position).or_insert(0.0);
            *total_score += simulated_score as f64;

            // Calculate UCB score
            let exploration_param = 2.0;
            let prior_prob = policy.i((0, position as i64)).double_value(&[]);
            let average_score = *total_score / (*visits as f64);
            let ucb_score = average_score
                + exploration_param
                * (prior_prob * ((total_visits as f64).ln() / (*visits as f64)).sqrt());

            ucb_scores.insert(position, ucb_score);
        }
    }

    // Return the position with the highest UCB score
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


/// Self-play training loop
async fn run_simulations_with_training(
    num_games: usize,
    num_simulations: usize,
    listener: TcpListener,
    policy_value_net: &PolicyValueNet<'_>,
    optimizer: &mut nn::Optimizer,
) {
    while let Ok((stream, _)) = listener.accept().await {
        let ws_stream = accept_async(stream).await.expect("Failed to accept WebSocket");
        let (mut write, _) = ws_stream.split();
        let mut scores_by_position: HashMap<usize, Vec<i32>> = HashMap::new();

        for game in 0..num_games {
            let mut deck = create_shuffle_deck();
            let mut plateau = create_plateau_empty();
            let mut game_data = Vec::new();
            let mut first_move: Option<(usize, Tile)> = None;

            println!("Starting game {}", game + 1);

            while !is_plateau_full(&plateau) {
                let tile_index = thread_rng().gen_range(0..deck.tiles.len());
                let chosen_tile = deck.tiles[tile_index];

                if let Some(best_position) = mcts_find_best_position_for_tile_with_nn(
                    &mut plateau,
                    &mut deck,
                    chosen_tile,
                    &policy_value_net,
                    num_simulations,
                ) {
                    if first_move.is_none() {
                        first_move = Some((best_position, chosen_tile));
                    }
                    plateau.tiles[best_position] = chosen_tile;
                    deck = replace_tile_in_deck(&deck, &chosen_tile);

                    // Save state, policy, and value for training
                    let board_tensor = convert_plateau_to_tensor(&plateau, &chosen_tile, &mut deck);
                    let (policy_logits, value) = policy_value_net.forward(&board_tensor);
                    game_data.push((board_tensor, policy_logits, value));

                    let image_names = generate_tile_image_names(&plateau.tiles);
                    let serialized = serde_json::to_string(&image_names).unwrap();
                    write.send(Message::Text(serialized)).await.unwrap();
                }
            }

            let final_score = result(&plateau);
            if let Some((position, _)) = first_move {
                scores_by_position
                    .entry(position)
                    .or_insert_with(Vec::new)
                    .push(final_score);
            }
            train_network_with_game_data(&game_data, final_score, policy_value_net, optimizer);
            println!("Game {} finished with score: {}", game + 1, final_score);
        }
        // Calculate and display averages
        let mut averages: Vec<(usize, f64)> = scores_by_position
            .iter()
            .map(|(position, scores)| {
                let average_score: f64 = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
                (*position, average_score)
            })
            .collect();

        // Sort averages by score
        averages.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Display sorted averages
        println!("\n--- Average Scores by First Position (Sorted) ---");
        for (position, average_score) in averages {
            println!("Position: {}, Average Score: {:.2}", position, average_score);
        }
        break;
    }
}

/// Train the neural network with game data
fn train_network_with_game_data(
    game_data: &[(Tensor, Tensor, Tensor)],
    final_score: i32,
    policy_value_net: &PolicyValueNet,
    optimizer: &mut nn::Optimizer,
) {
    let final_value = Tensor::of_slice(&[final_score as f32]);
    let loss = game_data.iter().fold(Tensor::zeros(&[], tch::kind::FLOAT_CPU), |loss, (state, policy, value)| {
        let (pred_policy, pred_value) = policy_value_net.forward(state);
        let policy_loss = -(policy * pred_policy.log()).sum(tch::Kind::Float);
        let value_loss = (final_value.shallow_clone() - pred_value).pow(&Tensor::of_slice(&[2.0]));
        loss + policy_loss + value_loss
    });

    optimizer.backward_step(&loss);
}


/// Converts the plateau, the chosen tile, and the remaining deck into a tensor representation.
///
/// - `plateau`: The game board with 19 tiles.
/// - `tile`: The chosen tile to be placed.
/// - `deck`: The current deck of remaining tiles.
///
/// Returns a tensor combining plateau, tile, and deck features.
// fn convert_plateau_to_tensor(plateau: &Plateau, tile: &Tile, deck: &Deck) -> Tensor {
//     let mut features = Vec::new();
//     // Add plateau features
//     for t in &plateau.tiles {
//         features.push(t.0 as f32);
//         features.push(t.1 as f32);
//         features.push(t.2 as f32);
//     }
//
//     // Add chosen tile features
//     features.push(tile.0 as f32);
//     features.push(tile.1 as f32);
//     features.push(tile.2 as f32);
//
//     // Add deck features
//     for t in &deck.tiles {
//         features.push(t.0 as f32);
//         features.push(t.1 as f32);
//         features.push(t.2 as f32);
//     }
//
//     // Check if the size matches the expected size (141)
//     assert_eq!(features.len(), 141, "Input tensor size does not match expected size!");
//
//     Tensor::of_slice(&features).unsqueeze(0) // Add batch dimension
// }
fn convert_plateau_to_tensor(plateau: &Plateau, tile: &Tile, deck: &Deck) -> Tensor {
    let mut features = vec![0.0; 3 * 5 * 5];

    for (i, t) in plateau.tiles.iter().enumerate() {
        let row = i / 5;
        let col = i % 5;
        features[row * 5 + col] = t.0 as f32 / 10.0; // Normalize
        features[25 + row * 5 + col] = t.1 as f32 / 10.0; // Normalize
        features[50 + row * 5 + col] = t.2 as f32 / 10.0; // Normalize
    }

    features[2 * 5 + 2] = tile.0 as f32 / 10.0; // Normalize
    features[25 + 2 * 5 + 2] = tile.1 as f32 / 10.0; // Normalize
    features[50 + 2 * 5 + 2] = tile.2 as f32 / 10.0; // Normalize

    Tensor::of_slice(&features).view([1, 3, 5, 5])
}


/// Train and evaluate the PolicyValueNet model with MCTS-based self-play and performance tracking.
///
/// - `policy_value_net`: The PolicyValueNet model to train.
/// - `optimizer`: The optimizer for training the neural network.
/// - `num_games`: Number of games to simulate per training iteration.
/// - `num_simulations`: Number of MCTS simulations per move.
/// - `evaluation_interval`: Number of games between evaluations.
/// - `listener`: WebSocket listener for communicating game states.
/// Train and evaluate the PolicyValueNet model with MCTS-based self-play and performance tracking.
///
/// - `policy_value_net`: The PolicyValueNet model to train.
/// - `optimizer`: The optimizer for training the neural network.
/// - `num_games`: Number of games to simulate per training iteration.
/// - `num_simulations`: Number of MCTS simulations per move.
/// - `evaluation_interval`: Number of games between evaluations.
/// - `listener`: WebSocket listener for communicating game states.
async fn train_and_evaluate(
    policy_value_net: &mut PolicyValueNet<'_>,
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
                    let tile_index = thread_rng().gen_range(0..deck.tiles.len());
                    let chosen_tile = deck.tiles[tile_index];

                    if let Some(best_position) = mcts_find_best_position_for_tile_with_nn(
                        &mut plateau,
                        &mut deck,
                        chosen_tile,
                        policy_value_net,
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
                        let (policy_logits, value) =
                            policy_value_net.forward(&board_tensor);
                        game_data.push((board_tensor, policy_logits, value));

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

                train_network_with_game_data(
                    &game_data,
                    final_score,
                    policy_value_net,
                    optimizer,
                );
                println!("Game {} finished with score: {}", game + 1, final_score);
                scores.push(final_score);
                if game % evaluation_interval_average == 0 {
                    let moyenne: f64 = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
                    println!("Partie {} - Score moyen: {:.2}", game, moyenne);
                    write.send(Message::Text(format!("GAME_RESULT:{}", moyenne))).await.unwrap();

                }
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

            // Pass a cloned Arc for evaluation
            evaluate_model(policy_value_net, num_simulations, Arc::clone(&listener)).await;

            println!(
                "Games Played: {}, Total Score: {}, Avg Score: {:.2}",
                games_played,
                total_score,
                total_score as f32 / games_played as f32
            );
        }

        // Exit after handling one connection
        break;
    }
}

async fn evaluate_model(
    policy_value_net: &PolicyValueNet<'_>,
    num_simulations: usize,
    listener: Arc<TcpListener>,
) {
    println!("Evaluating model...");
    // Simulate a fixed number of games to test performance
    let mut scores = Vec::new();

    for _ in 0..10 {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();

        while !is_plateau_full(&plateau) {
            let tile_index = thread_rng().gen_range(0..deck.tiles.len());
            let chosen_tile = deck.tiles[tile_index];

            if let Some(best_position) = mcts_find_best_position_for_tile_with_nn(
                &mut plateau,
                &mut deck,
                chosen_tile,
                policy_value_net,
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




async fn run_simulations(num_games: usize, num_simulations: usize, listener: TcpListener) {
    let mut scores = Vec::new();
    let seed = [42u8; 32];
    let mut rng = StdRng::from_seed(seed);
    while let Ok((stream, _)) = listener.accept().await {
        let ws_stream = accept_async(stream).await.expect("Failed to accept WebSocket");
        let (mut write, _) = ws_stream.split();
        let mut scores_by_position: HashMap<usize, Vec<i32>> = HashMap::new();

        for game in 0..num_games {
            let mut deck = create_shuffle_deck();
            let mut plateau = create_plateau_empty();
            let mut first_move: Option<(usize, Tile)> = None;

            println!("Starting game {}", game + 1);

            while !is_plateau_full(&plateau) {
                let tile_index = rng.gen_range(0..deck.tiles.len());
                let chosen_tile = deck.tiles[tile_index];

                if let Some(best_position) = mcts_find_best_position_for_tile(&mut plateau, &mut deck, chosen_tile, num_simulations) {
                    if first_move.is_none() {
                        first_move = Some((best_position, chosen_tile));
                    }
                    plateau.tiles[best_position] = chosen_tile;
                    deck = remove_tile_from_deck(&deck, &chosen_tile);

                    let image_names = generate_tile_image_names(&plateau.tiles);
                    let serialized = serde_json::to_string(&image_names).unwrap();
                    write.send(Message::Text(serialized)).await.unwrap();
                }
            }

            let game_score = result(&plateau);
            scores.push(game_score);
            if let Some((position, _)) = first_move {
                scores_by_position.entry(position).or_insert_with(Vec::new).push(game_score);
            }
            println!("Game {} finished with score: {}", game + 1, game_score);
        }

        let mut averages: Vec<(usize, f64)> = scores_by_position
            .iter()
            .map(|(position, scores)| {
                let average_score: f64 = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
                (*position, average_score)
            })
            .collect();
        averages.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        println!("\n--- Average Scores by First Position (Sorted) ---");
        for (position, average_score) in averages {
            println!("Position: {}, Average Score: {:.2}", position, average_score);
        }
    }
}

/// Checks if the plateau is full
fn is_plateau_full(plateau: &Plateau) -> bool {
    plateau.tiles.iter().all(|tile| *tile != Tile(0, 0, 0))
}

/// Finds the best move using MCTS
fn mcts_find_best_position_for_tile(plateau: &mut Plateau, deck: &mut Deck, chosen_tile: Tile, num_simulations: usize) -> Option<usize> {
    let mut best_position = None;
    let mut best_score = i32::MIN;

    for position in get_legal_moves(plateau.clone()) {
        let mut temp_plateau = plateau.clone();
        let mut temp_deck = deck.clone();

        // Simulate placing the tile at this position
        temp_plateau.tiles[position] = chosen_tile;
        temp_deck = remove_tile_from_deck(&temp_deck, &chosen_tile);

        // Simulate games to evaluate the score
        let score = simulate_games(temp_plateau.clone(), temp_deck.clone(), num_simulations);

        if score > best_score {
            best_score = score;
            best_position = Some(position);
        }
    }

    best_position
}

/// Simulates `num_simulations` games and returns the average score
fn simulate_games(mut plateau: Plateau, mut deck: Deck, num_simulations: usize) -> i32 {
    let mut total_score = 0;
    let mut rng = thread_rng();
    for _ in 0..num_simulations {
        let mut simulated_plateau = plateau.clone();
        let mut simulated_deck = deck.clone();
        let mut legal_moves = get_legal_moves(simulated_plateau.clone());
        while !is_plateau_full(&simulated_plateau) {

            if legal_moves.is_empty() {
                break;
            }


            let position = legal_moves[rng.gen_range(0..legal_moves.len())];
            let tile_index = rng.gen_range(0..simulated_deck.tiles.len());
            let chosen_tile = simulated_deck.tiles[tile_index];
            if let Some(index) = legal_moves.iter().position(|&x| x == position) {
                legal_moves.swap_remove(index);
            }
            simulated_plateau.tiles[position] = chosen_tile;
            simulated_deck = replace_tile_in_deck(&simulated_deck, &chosen_tile);

        }

        total_score += result(&simulated_plateau);
    }

    total_score / num_simulations as i32
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
