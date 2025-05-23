use crate::create_shuffle_deck::create_shuffle_deck;
use crate::create_plateau_empty::create_plateau_empty;
use crate::remove_tile_from_deck::replace_tile_in_deck;
use crate::generate_tile_image_names;
use crate::result::result;
use crate::test::{Deck, Plateau, Tile};
use crate::{mcts_find_best_position_for_tile_with_nn, is_plateau_full};

use serde_json::json;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::WebSocketStream;
use tokio::net::TcpStream;
use futures_util::{StreamExt, SinkExt};
use futures_util::stream::SplitSink;
use rand::Rng;
use crate::policy_value_net::{PolicyNet, ValueNet};

pub async fn play_mcts_vs_human(
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    num_simulations: usize,
    write: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
    read: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
) {
    let mut deck = create_shuffle_deck();
    let mut plateau_human = create_plateau_empty();
    let mut plateau_mcts = create_plateau_empty();

    let total_turns = 19;
    let mut current_turn = 0;

    while !is_plateau_full(&plateau_human) {
        let tile_index = rand::rng().random_range(0..deck.tiles.len());
        let tile = deck.tiles[tile_index];
        deck = replace_tile_in_deck(&deck, &tile); // remove it from deck
        let tile_image = format!("../image/{}{}{}.png", tile.0, tile.1, tile.2);

        // ✅ Send state to frontend
        let payload = json!({
            "type": "mcts_vs_human_turn",
            "tile": tile_image,
            "plateau_human": generate_tile_image_names(&plateau_human.tiles),
            "plateau_mcts": generate_tile_image_names(&plateau_mcts.tiles)
        });
        if write.send(Message::Text(payload.to_string())).await.is_err() {
            eprintln!("❌ Failed to send turn info to frontend (WebSocket closed?)");
            return;
        }

        // ✅ Wait for HUMAN move
        let mut human_move: Option<usize> = None;
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Some(pos_str) = text.strip_prefix("HUMAN_MOVE:") {
                        if let Ok(pos) = pos_str.trim().parse::<usize>() {
                            human_move = Some(pos);
                            break;
                        }
                    }
                }
                Ok(Message::Close(_)) | Err(_) => {
                    eprintln!("❌ WebSocket connection closed or errored during input.");
                    return;
                }
                _ => {}
            }
        }

        if let Some(pos) = human_move {
            if plateau_human.tiles[pos] == Tile(0, 0, 0) {
                plateau_human.tiles[pos] = tile;
            } else {
                eprintln!("⚠️ Human tried to play on non-empty tile. Ignoring.");
                continue; // Skip to the next loop iteration
            }
        } else {
            eprintln!("❌ No valid move received from human.");
            break;
        }

        // ✅ MCTS plays
        let mut deck_clone = deck.clone(); // don't mutate original deck
        let mcts_result = mcts_find_best_position_for_tile_with_nn(
            &mut plateau_mcts,
            &mut deck_clone,
            tile,
            policy_net,
            value_net,
            num_simulations,
            current_turn,
            total_turns,
        );
        plateau_mcts.tiles[mcts_result.best_position] = tile;

        current_turn += 1;
    }

    // ✅ Game over - send results
    let score_human = result(&plateau_human);
    let score_mcts = result(&plateau_mcts);

    let final_payload = json!({
        "type": "mcts_vs_human_result",
        "score_human": score_human,
        "score_mcts": score_mcts
    });

    if write.send(Message::Text(final_payload.to_string())).await.is_err() {
        eprintln!("❌ Failed to send final results.");
    }
}
