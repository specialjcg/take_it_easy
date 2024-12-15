use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::protocol::Message;
use serde_json;
use rand::{Rng, SeedableRng, thread_rng};
use rand::rngs::StdRng;
use result::result;
use crate::test::{create_plateau_empty, create_shuffle_deck, Deck, Plateau, remove_tile_from_deck, Tile};


mod test;
mod result;

fn generate_tile_image_names(tiles: &[Tile]) -> Vec<String> {
    tiles.iter().map(|tile| {
        format!("../image/{}{}{}.png", tile.0, tile.1, tile.2)
    }).collect()
}

#[tokio::main]
async fn main() {
    let seed = [42u8; 32];
    let mut rng = StdRng::from_seed(seed);

    // WebSocket server setup (optional if you want to visualize live games)
    let listener = TcpListener::bind("127.0.0.1:9000")
        .await
        .expect("Failed to bind WebSocket server");
    println!("WebSocket server running at ws://127.0.0.1:9000");

    // Collect statistics
    let mut scores = Vec::new();

    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let ws_stream = accept_async(stream).await.expect("Failed to accept WebSocket");
            let (mut write, _) = ws_stream.split();

            for game in 0..10 {
                // Initialize the plateau and deck
                let mut deck = create_shuffle_deck();
                let mut plateau = create_plateau_empty();
                let mut first_move: Option<(usize, Tile)> = None; // To track the first move


                println!("Starting game {}", game + 1);

                while !is_plateau_full(&plateau) {
                    // Select the best tile and position using MCTS
                    if let Some((best_position, chosen_tile)) = mcts_find_best_move(&mut deck, &mut plateau) {
                        if first_move.is_none() {
                            first_move = Some((best_position, chosen_tile));
                        }
                        // Place the tile on the plateau
                        plateau.tiles[best_position] = chosen_tile;

                        // Remove the chosen tile from the deck
                        deck = remove_tile_from_deck(&deck, &chosen_tile);

                        // Track (position, tile) pair frequency


                        // Generate image filenames for the plateau
                        let image_names = generate_tile_image_names(&plateau.tiles);

                        // Serialize image names to JSON and send to WebSocket
                        let serialized = serde_json::to_string(&image_names).unwrap();
                        write.send(Message::Text(serialized)).await.unwrap();


                    }
                }

                // Calculate the score for the current game
                let game_score = result(&plateau);
                scores.push(game_score);

                if let Some((position, tile)) = first_move {
                    println!(
                        "Game {} finished with score: {}, First move: Tile {:?} at Position {}",
                        game + 1,
                        game_score,
                        tile,
                        position
                    );
                } else {
                    println!("Game {} finished with score: {}. No valid moves were made.", game + 1, game_score);
                }
            }

            // Post-game analysis
        }
    })
        .await
        .unwrap();
}





/// Checks if the plateau is full
fn is_plateau_full(plateau: &Plateau) -> bool {
    plateau.tiles.iter().all(|tile| *tile != Tile(0, 0, 0))
}

/// Finds the best move using MCTS
fn mcts_find_best_move(deck: &mut Deck, plateau: &mut Plateau) -> Option<(usize, Tile)> {
    let mut best_position = None;
    let mut best_score = i32::MIN;
    let mut chosen_tile = None;

    for tile in &deck.tiles {
        for position in get_legal_moves(plateau.clone()) {
            let mut temp_plateau = plateau.clone();
            let mut temp_deck = deck.clone();

            // Simulate placing the tile
            temp_plateau.tiles[position] = *tile;
            temp_deck = remove_tile_from_deck(&temp_deck, tile);

            // Simulate games to evaluate the score
            let score = simulate_games(temp_plateau.clone(), temp_deck.clone(), 100);

            if score > best_score {
                best_score = score;
                best_position = Some(position);
                chosen_tile = Some(*tile);
            }
        }
    }

    best_position.zip(chosen_tile)
}

/// Simulates `num_simulations` games and returns the average score
fn simulate_games(mut plateau: Plateau, mut deck: Deck, num_simulations: usize) -> i32 {
    let mut total_score = 0;

    for _ in 0..num_simulations {
        let mut simulated_plateau = plateau.clone();
        let mut simulated_deck = deck.clone();

        while !is_plateau_full(&simulated_plateau) {
            let legal_moves = get_legal_moves(simulated_plateau.clone());
            if legal_moves.is_empty() {
                break;
            }

            let mut rng = thread_rng();
            let position = legal_moves[rng.gen_range(0..legal_moves.len())];
            let tile_index = rng.gen_range(0..simulated_deck.tiles.len());
            let chosen_tile = simulated_deck.tiles[tile_index];

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
