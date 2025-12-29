//! Test UCT MCTS to verify it creates NON-UNIFORM distributions
//! 
//! This test confirms that the UCT approach solves the circular learning problem
//! by showing that the policy network influences exploration.

use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::prelude::IndexedRandom;
use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_uct;
use take_it_easy::mcts::hyperparameters::MCTSHyperparameters;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::scoring::scoring::result;

fn chi_squared_test(observed: &[usize], expected: f64) -> f64 {
    observed
        .iter()
        .map(|&obs| {
            let diff = obs as f64 - expected;
            (diff * diff) / expected
        })
        .sum()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing UCT MCTS Distribution\n");
    println!("Goal: Verify that UCT creates NON-UNIFORM position selection");
    println!("(Unlike batch MCTS which always has chi² ≈ 0.00)\n");

    // Create neural network (with trained weights if available)
    println!("📂 Loading neural network...");
    let neural_config = NeuralConfig {
        input_dim: (8, 5, 5),
        nn_architecture: NNArchitecture::CNN,
        ..Default::default()
    };

    let mut manager = NeuralManager::with_config(neural_config)?;
    println!("   ✅ Network loaded\n");

    // Test parameters
    let num_games = 100;
    let simulations = 150;
    let mut rng = StdRng::seed_from_u64(2025);
    let hyperparams = MCTSHyperparameters::default();

    // Track position selections
    let mut position_counts = vec![0usize; 19];
    let mut total_positions = 0;
    let mut scores: Vec<i32> = Vec::new();

    println!("🎮 Running {} games with UCT MCTS ({} sims)...\n", num_games, simulations);

    for game_idx in 0..num_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..19 {
            let available = get_available_tiles(&deck);
            if available.is_empty() {
                break;
            }

            let chosen_tile = *available.choose(&mut rng).unwrap();
            deck = replace_tile_in_deck(&deck, &chosen_tile);

            let policy_net = manager.policy_net();
            let value_net = manager.value_net();

            let mcts_result = mcts_find_best_position_for_tile_uct(
                &mut plateau,
                &mut deck,
                chosen_tile,
                policy_net,
                value_net,
                simulations,
                turn,
                19,
                Some(&hyperparams),
            );

            let position = mcts_result.best_position;
            plateau.tiles[position] = chosen_tile;

            position_counts[position] += 1;
            total_positions += 1;
        }

        let score = result(&plateau);
        scores.push(score);

        if (game_idx + 1) % 20 == 0 {
            let recent_mean = scores[scores.len().saturating_sub(20)..].iter().sum::<i32>() as f64
                / scores.len().saturating_sub(scores.len() - 20).max(1) as f64;
            println!("  Game {}/{}: Recent avg = {:.2} pts", game_idx + 1, num_games, recent_mean);
        }
    }

    // Calculate statistics
    let mean_score = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
    let std_dev = (scores.iter()
        .map(|&s| (s as f64 - mean_score).powi(2))
        .sum::<f64>() / scores.len() as f64)
        .sqrt();

    // Calculate chi-squared statistic
    let expected = total_positions as f64 / 19.0;
    let chi_squared = chi_squared_test(&position_counts, expected);

    // Results
    println!("\n{}", "=".repeat(60));
    println!("RESULTS");
    println!("{}", "=".repeat(60));
    println!("\n📊 Performance:");
    println!("  Mean score: {:.2} ± {:.2} pts", mean_score, std_dev);
    println!("  Total moves: {}", total_positions);

    println!("\n📈 Position Distribution:");
    println!("  Expected per position: {:.1}", expected);
    println!("  Chi-squared statistic: {:.2}", chi_squared);

    // Show top 5 and bottom 5 positions
    let mut sorted_positions: Vec<_> = position_counts.iter().enumerate().collect();
    sorted_positions.sort_by(|a, b| b.1.cmp(a.1));

    println!("\n  Most selected positions:");
    for (pos, &count) in sorted_positions.iter().take(5) {
        println!("    Position {}: {} times ({:.1}%)", pos, count, 
                 count as f64 / total_positions as f64 * 100.0);
    }

    println!("\n  Least selected positions:");
    for (pos, &count) in sorted_positions.iter().rev().take(5) {
        println!("    Position {}: {} times ({:.1}%)", pos, count,
                 count as f64 / total_positions as f64 * 100.0);
    }

    // Interpretation
    println!("\n🔍 Interpretation:");
    if chi_squared < 5.0 {
        println!("  ❌ UNIFORM distribution (chi² = {:.2} < 5.0)", chi_squared);
        println!("  Policy network is NOT influencing exploration");
        println!("  UCT is selecting positions uniformly (problem persists)");
    } else if chi_squared < 20.0 {
        println!("  ⚠️  WEAKLY NON-UNIFORM (chi² = {:.2})", chi_squared);
        println!("  Some preference detected, but still mostly uniform");
    } else {
        println!("  ✅ STRONGLY NON-UNIFORM (chi² = {:.2} > 20.0)", chi_squared);
        println!("  Policy network IS influencing exploration!");
        println!("  UCT successfully breaks the circular learning problem");
    }

    // Critical degrees of freedom test (19 positions - 1 = 18 df)
    // p < 0.05 threshold: chi² > 28.87
    // p < 0.01 threshold: chi² > 34.81
    println!("\n📐 Statistical significance (18 degrees of freedom):");
    if chi_squared > 34.81 {
        println!("  ✅ Highly significant (p < 0.01): chi² = {:.2} > 34.81", chi_squared);
    } else if chi_squared > 28.87 {
        println!("  ✅ Significant (p < 0.05): chi² = {:.2} > 28.87", chi_squared);
    } else {
        println!("  ❌ Not significant: chi² = {:.2} < 28.87", chi_squared);
        println!("  Cannot reject uniform distribution hypothesis");
    }

    Ok(())
}
