//! Compare MCTS vs Greedy to identify where MCTS fails

use flexi_logger::Logger;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::prelude::IndexedRandom;
use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::tensor_conversion::convert_plateau_to_tensor;
use take_it_easy::scoring::scoring::result;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    log::info!("🔍 MCTS vs Greedy Comparison");

    // Initialize network
    let neural_config = NeuralConfig {
        input_dim: (9, 5, 5),
        nn_architecture: NNArchitecture::Cnn,
        ..Default::default()
    };
    let manager = NeuralManager::with_config(neural_config)?;

    let num_simulations = 150;

    let mut rng = StdRng::seed_from_u64(2025);

    // Play ONE game with both methods
    log::info!("\n=== Playing Game with Both Methods ===\n");

    let mut plateau_greedy = create_plateau_empty();
    let mut plateau_mcts = create_plateau_empty();
    let mut deck = create_deck();

    for turn in 0..19 {
        let available = get_available_tiles(&deck);
        if available.is_empty() {
            break;
        }

        let chosen_tile = *available.choose(&mut rng).unwrap();
        deck = replace_tile_in_deck(&deck, &chosen_tile);

        // === GREEDY SELECTION ===
        let state_tensor = convert_plateau_to_tensor(&plateau_greedy, &chosen_tile, &deck, turn, 19);
        let policy_pred = manager.policy_net().forward(&state_tensor, false);
        let policy_probs = policy_pred.softmax(-1, tch::Kind::Float);
        let probs_data: Vec<f32> = Vec::try_from(policy_probs.squeeze_dim(0))?;

        let valid_positions: Vec<usize> = plateau_greedy.tiles.iter()
            .enumerate()
            .filter(|(_, tile)| **tile == Tile(0, 0, 0))
            .map(|(idx, _)| idx)
            .collect();

        let (greedy_pos, greedy_prob) = valid_positions.iter()
            .map(|&pos| (pos, probs_data[pos]))
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();

        // === MCTS SELECTION ===
        let mcts_result = mcts_find_best_position_for_tile_with_nn(
            &mut plateau_mcts,
            &mut deck,
            chosen_tile,
            manager.policy_net(),
            manager.value_net(),
            num_simulations,
            turn,
            19,
            None,  // Use default hyperparameters
        );
        let mcts_pos = mcts_result.best_position;

        // Get MCTS probability for comparison
        let mcts_prob = probs_data[mcts_pos];

        log::info!("Turn {}: tile={:?}", turn + 1, chosen_tile);
        log::info!("  Greedy: pos={} prob={:.4}", greedy_pos, greedy_prob);
        log::info!("  MCTS:   pos={} prob={:.4}", mcts_pos, mcts_prob);

        if greedy_pos != mcts_pos {
            log::info!("  ⚠️  MCTS chose DIFFERENT position!");

            // Show top 3 positions according to policy
            let mut top_positions: Vec<(usize, f32)> = valid_positions.iter()
                .map(|&pos| (pos, probs_data[pos]))
                .collect();
            top_positions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            log::info!("  Top 3 by policy:");
            for (i, (pos, prob)) in top_positions.iter().take(3).enumerate() {
                log::info!("    {}. pos={} prob={:.4}", i+1, pos, prob);
            }
        }

        // Place tiles
        plateau_greedy.tiles[greedy_pos] = chosen_tile;
        plateau_mcts.tiles[mcts_pos] = chosen_tile;

        log::info!("");
    }

    let score_greedy = result(&plateau_greedy);
    let score_mcts = result(&plateau_mcts);

    log::info!("\n📊 Final Results:");
    log::info!("  Greedy score: {}", score_greedy);
    log::info!("  MCTS score:   {}", score_mcts);
    log::info!("  Difference:   {}", score_greedy as i32 - score_mcts as i32);

    if score_greedy > score_mcts {
        log::info!("\n❌ MCTS performed WORSE than greedy!");
    } else if score_mcts > score_greedy {
        log::info!("\n✅ MCTS performed BETTER than greedy!");
    } else {
        log::info!("\n🤝 Same performance");
    }

    Ok(())
}
