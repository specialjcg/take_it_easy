//! Test greedy play - select best position according to policy network

use flexi_logger::Logger;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::prelude::IndexedRandom;
use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::tensor_conversion::convert_plateau_to_tensor;
use take_it_easy::scoring::scoring::result;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    log::info!("🎮 Greedy Play Test (No MCTS - Direct Policy)");

    // Initialize network
    let neural_config = NeuralConfig {
        input_dim: (9, 5, 5),
        nn_architecture: NNArchitecture::Cnn,
        ..Default::default()
    };
    let manager = NeuralManager::with_config(neural_config)?;

    let mut rng = StdRng::seed_from_u64(2025);
    let num_games = 10;
    let mut scores = Vec::new();

    for game_idx in 0..num_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        log::info!("\n=== Game {} ===", game_idx + 1);

        for turn in 0..19 {
            let available = get_available_tiles(&deck);
            if available.is_empty() {
                break;
            }

            let chosen_tile = *available.choose(&mut rng).unwrap();
            deck = replace_tile_in_deck(&deck, &chosen_tile);

            // Get policy prediction
            let state_tensor = convert_plateau_to_tensor(&plateau, &chosen_tile, &deck, turn, 19);
            let policy_pred = manager.policy_net().forward(&state_tensor, false);
            let policy_probs = policy_pred.softmax(-1, tch::Kind::Float);
            let probs_data: Vec<f32> = Vec::try_from(policy_probs.squeeze_dim(0))?;

            // Find valid positions (empty cells)
            let valid_positions: Vec<usize> = plateau.tiles.iter()
                .enumerate()
                .filter(|(_, tile)| **tile == take_it_easy::game::tile::Tile(0, 0, 0))
                .map(|(idx, _)| idx)
                .collect();

            // Select position with highest probability among valid positions
            let (best_pos, best_prob) = valid_positions.iter()
                .map(|&pos| (pos, probs_data[pos]))
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .unwrap();

            log::info!("Turn {}: tile={:?}, chose pos {} (prob={:.4})",
                turn + 1, chosen_tile, best_pos, best_prob);

            // Place tile
            plateau.tiles[best_pos] = chosen_tile;
        }

        let score = result(&plateau);
        scores.push(score);
        log::info!("Game {} final score: {}", game_idx + 1, score);
    }

    let avg_score = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
    let min_score = *scores.iter().min().unwrap();
    let max_score = *scores.iter().max().unwrap();

    log::info!("\n📊 Greedy Play Results:");
    log::info!("   Games: {}", num_games);
    log::info!("   Average: {:.1}", avg_score);
    log::info!("   Min: {}", min_score);
    log::info!("   Max: {}", max_score);
    log::info!("\n   Expected good network: > 120 pts");
    log::info!("   Expected bad network: < 80 pts");

    Ok(())
}
