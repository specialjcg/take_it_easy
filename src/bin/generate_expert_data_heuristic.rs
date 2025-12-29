//! Generate expert training data using MCTS Pure (rollout-based, no neural network)
//!
//! Plays N games using MCTS with rollout evaluations only.
//! Saves game states and moves to bootstrap the policy/value networks.
//!
//! This breaks the circular learning problem by providing non-uniform initial data.

use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::prelude::IndexedRandom;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::neural::tensor_conversion::convert_plateau_to_tensor;
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_pure;
use take_it_easy::mcts::hyperparameters::MCTSHyperparameters;
use take_it_easy::scoring::scoring::result;

#[derive(Serialize, Deserialize)]
struct ExpertExample {
    /// Serialized board state tensor (flattened)
    state: Vec<f32>,
    /// Position selected by heuristic
    policy_target: i64,
    /// Final game score (filled in after game completes)
    value_target: f32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎓 Generating Expert Training Data (MCTS Pure - Rollout-Based)\n");

    let num_games = 200; // Reduced for faster iteration
    let turns_per_game = 19;
    let mcts_sims = 100; // Balanced for quality vs speed

    let mut rng = StdRng::seed_from_u64(2025);
    let mut all_examples: Vec<ExpertExample> = Vec::new();
    let mut scores: Vec<i32> = Vec::new();
    let hyperparams = MCTSHyperparameters::default();

    println!("🎮 Playing {} games with MCTS Pure ({} sims per move)...\n", num_games, mcts_sims);

    for game_idx in 0..num_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut game_examples: Vec<ExpertExample> = Vec::new();

        for turn in 0..turns_per_game {
            let available = get_available_tiles(&deck);
            if available.is_empty() {
                break;
            }

            // Random tile selection (like real game)
            let chosen_tile = *available.choose(&mut rng).unwrap();
            deck = replace_tile_in_deck(&deck, &chosen_tile);

            // **MCTS Pure Selection** (rollouts only, no neural network)
            let mcts_result = mcts_find_best_position_for_tile_pure(
                &mut plateau,
                &mut deck,
                chosen_tile,
                mcts_sims,
                turn,
                turns_per_game,
                Some(&hyperparams),
            );

            let best_position = mcts_result.best_position;

            // Record this state and move
            let state_tensor = convert_plateau_to_tensor(
                &plateau,
                &chosen_tile,
                &deck,
                turn,
                turns_per_game,
            );

            // Flatten tensor to Vec<f32> for serialization
            let state_vec: Vec<f32> = state_tensor.view([-1])
                .try_into()
                .unwrap();

            game_examples.push(ExpertExample {
                state: state_vec,
                policy_target: best_position as i64,
                value_target: 0.0, // Will fill in after game ends
            });

            // Make the move
            plateau.tiles[best_position] = chosen_tile;
        }

        // Game finished, get final score
        let final_score = result(&plateau);
        scores.push(final_score);

        // Normalize score to [-1, 1] range for value target
        let normalized_score = ((final_score as f32 / 200.0).clamp(0.0, 1.0) * 2.0) - 1.0;

        // Fill in value targets for all moves in this game
        for example in &mut game_examples {
            example.value_target = normalized_score;
        }

        all_examples.extend(game_examples);

        if (game_idx + 1) % 20 == 0 {
            let recent_mean = scores[scores.len().saturating_sub(20)..]
                .iter()
                .sum::<i32>() as f32
                / scores.len().saturating_sub(scores.len() - 20).max(1) as f32;
            println!("  Game {}/{}: Recent 20 avg = {:.2} pts",
                     game_idx + 1, num_games, recent_mean);
        }
    }

    let mean_score = scores.iter().sum::<i32>() as f32 / scores.len() as f32;
    let std_dev = (scores.iter()
        .map(|&s| (s as f32 - mean_score).powi(2))
        .sum::<f32>() / scores.len() as f32)
        .sqrt();

    println!("\n📊 Expert Data Statistics:");
    println!("  Total examples: {}", all_examples.len());
    println!("  Mean score: {:.2} ± {:.2} pts", mean_score, std_dev);
    println!("  Examples per game: {:.1}", all_examples.len() as f32 / num_games as f32);

    // Save to JSON file
    let output_path = "expert_data_mcts_pure.json";
    let json = serde_json::to_string(&all_examples)?;
    let mut file = File::create(output_path)?;
    file.write_all(json.as_bytes())?;

    println!("\n✅ Saved {} examples to {}", all_examples.len(), output_path);
    println!("   File size: {:.2} MB", json.len() as f64 / 1_000_000.0);

    Ok(())
}
