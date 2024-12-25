use std::collections::HashMap;
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
use tch::{nn, Tensor};
use tch::nn::OptimizerConfig;
use crate::policy_value_net::PolicyValueNet;

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

    let policy_value_net = PolicyValueNet::new(57 + 3 + 81, 128); // Adjust input/output sizes
    let mut optimizer = nn::Adam::default().build(&policy_value_net.vs, 1e-3).unwrap();

    // Rest of your main function
    let listener = TcpListener::bind("127.0.0.1:9000")
        .await
        .expect("Failed to bind WebSocket server");
    println!("WebSocket server running at ws://127.0.0.1:9000");

    // Pass parameters where needed
    run_simulations(config.num_games, config.num_simulations, listener).await;
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

    let mut ucb_scores = HashMap::new();
    for _ in 0..num_simulations {
        for &position in &legal_moves {
            let mut temp_plateau = plateau.clone();
            let mut temp_deck = deck.clone();

            // Simulate placing the tile at this position
            temp_plateau.tiles[position] = chosen_tile;
            temp_deck = remove_tile_from_deck(&temp_deck, &chosen_tile);

            // Simulate games to evaluate the score
            let simulated_score = simulate_games(temp_plateau.clone(), temp_deck.clone(), 1);

            let visits = 1.0; // Replace with real visit count tracking if needed
            let exploration_param = 1.4;
            let prior_prob = policy.double_value(&[position as i64]);
            let ucb_score = (simulated_score as f64) / visits
                + exploration_param * (prior_prob * ((1.0 / visits).ln() + 1.0).sqrt());
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
    policy_value_net: &PolicyValueNet,
    optimizer: &mut nn::Optimizer,
) {
    while let Ok((stream, _)) = listener.accept().await {
        let ws_stream = accept_async(stream).await.expect("Failed to accept WebSocket");
        let (mut write, _) = ws_stream.split();

        for game in 0..num_games {
            let mut deck = create_shuffle_deck();
            let mut plateau = create_plateau_empty();
            let mut game_data = Vec::new();

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
                    plateau.tiles[best_position] = chosen_tile;
                    deck = remove_tile_from_deck(&deck, &chosen_tile);

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
            train_network_with_game_data(&game_data, final_score, policy_value_net, optimizer);
            println!("Game {} finished with score: {}", game + 1, final_score);
        }
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
fn convert_plateau_to_tensor(plateau: &Plateau, tile: &Tile, deck: &Deck) -> Tensor {
    let mut features = Vec::new();

    // Flatten the plateau tiles into a vector of features (3 features per tile)
    for t in &plateau.tiles {
        features.push(t.0 as f32);
        features.push(t.1 as f32);
        features.push(t.2 as f32);
    }

    // Add the chosen tile features
    features.push(tile.0 as f32);
    features.push(tile.1 as f32);
    features.push(tile.2 as f32);

    // Add the deck features
    for t in &deck.tiles {
        features.push(t.0 as f32);
        features.push(t.1 as f32);
        features.push(t.2 as f32);
    }

    // Convert the feature vector into a tensor
    Tensor::of_slice(&features)
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

    for _ in 0..num_simulations {
        let mut simulated_plateau = plateau.clone();
        let mut simulated_deck = deck.clone();
        let mut legal_moves = get_legal_moves(simulated_plateau.clone());
        while !is_plateau_full(&simulated_plateau) {

            if legal_moves.is_empty() {
                break;
            }

            let mut rng = thread_rng();
            let position = legal_moves[rng.gen_range(0..legal_moves.len())];
            let tile_index = rng.gen_range(0..simulated_deck.tiles.len());
            let chosen_tile = simulated_deck.tiles[tile_index];
            if let Some(index) = legal_moves.iter().position(|&x| x == position) {
                legal_moves.swap_remove(index);
            }
            simulated_plateau.tiles[position] = chosen_tile;
            simulated_deck = remove_tile_from_deck(&simulated_deck, &chosen_tile);

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
