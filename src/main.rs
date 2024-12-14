use std::thread::sleep;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::protocol::Message;
use serde_json;
use rand::{thread_rng, Rng, SeedableRng};
use rand::rngs::{OsRng, StdRng};
use crate::test::{choisir_et_placer, create_plateau_empty, create_shuffle_deck, Plateau, result, Tile};


mod test;

/// Simule 100 parties à partir d'un état donné pour calculer le meilleur score
fn simulate_games(deck: &mut test::Deck, plateau: &mut Plateau) -> (i32, Plateau) {
    let mut best_score = 0;
    let mut best_plateau = plateau.clone();

    for _ in 0..20000 {
        let mut simulation_deck = deck.clone(); // Clone du deck
        let mut simulation_plateau = plateau.clone(); // Clone du plateau

        // Simule une partie complète avec les clones
        choisir_et_placer(&mut simulation_deck, &mut simulation_plateau);

        // Calcule le score pour cette partie
        let score = result(&simulation_plateau);

        // Met à jour si ce score est le meilleur
        if score > best_score {
            best_score = score;
            best_plateau = simulation_plateau.clone();
        }
    }

    // Retourne le meilleur score trouvé parmi les simulations
    (best_score,best_plateau)
}

/// Trouve la meilleure position pour placer une tuile en simulant 100 parties pour chaque position
fn find_best_position(deck: &mut test::Deck, plateau: &mut Plateau, tuile: Tile) -> (usize, i32, Plateau) {
    let mut best_score = 0;
    let mut best_position = 0;
    let mut best_plateau_score = plateau.clone();

    for position in 0..plateau.tiles.len() {
        // Vérifie si la case est vide
        if plateau.tiles[position] == Tile(0, 0, 0) {
            // Clone le plateau pour tester ce placement
            let mut temp_plateau = plateau.clone();
            temp_plateau.tiles[position] = tuile;

            // Simule 100 parties avec cette configuration
             let (best_position_score,best_plateau) = simulate_games(deck, &mut temp_plateau);

            // Met à jour si ce score est le meilleur
            if best_position_score > best_score {
                best_score = best_position_score;
                best_position = position;
                best_plateau_score = best_plateau.clone();
            }
        }
    }

    (best_position, best_score,best_plateau_score)
}
fn generate_tile_image_names(tiles: &[Tile]) -> Vec<String> {
    tiles.iter().map(|tile| {
        format!("../image/{}{}{}.png", tile.0, tile.1, tile.2)
    }).collect()
}

#[tokio::main]
async fn main() {
    let seed = [42u8; 32];
    let mut rng = StdRng::from_seed(seed);


    // Initialize the plateau and deck
    let mut deck = create_shuffle_deck();
    let mut plateau = create_plateau_empty();

    // WebSocket server setup
    let listener = TcpListener::bind("127.0.0.1:9000").await.expect("Failed to bind WebSocket server");
    println!("WebSocket server running at ws://127.0.0.1:9000");

    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let ws_stream = accept_async(stream).await.expect("Failed to accept WebSocket");
            let (mut write, _) = ws_stream.split();

            loop {
                if is_plateau_full(&plateau) {
                    let point=result(&plateau);
                    println!("The game has ended! The plateau is full. {}", point);
                    break;
                }

                // Choose a random tile from the deck
                let deck_len = deck.tiles.len();
                let tile_index = rng.gen_range(0..deck_len);
                let chosen_tile = deck.tiles[tile_index].clone();

                // Remove the tile from the deck
                deck = crate::test::remove_tile_from_deck(&deck, &chosen_tile);

                // Find the best position for the chosen tile
                let (best_position, best_score, _) = find_best_position(&mut deck, &mut plateau, chosen_tile);

                // Place the tile on the plateau
                plateau.tiles[best_position] = chosen_tile;

                // Generate image filenames for the plateau
                let image_names = generate_tile_image_names(&plateau.tiles);

                // Serialize image names to JSON
                let serialized = serde_json::to_string(&image_names).unwrap();
                write.send(Message::Text(serialized)).await.unwrap();

                println!("Tile placed: {:?}", chosen_tile);
                println!("Best position: {}", best_position);
                println!("Current plateau: {:?}", plateau.tiles);

                // Sleep for 1 second before the next placement
            }
        }
    })
        .await
        .unwrap();
}

/// Checks if the plateau is full
fn is_plateau_full(plateau: &Plateau) -> bool {
    plateau.tiles.iter().all(|tile| *tile != Tile(0, 0, 0))
}
