//! Compare Batch MCTS vs UCT MCTS on identical games
//! 
//! This investigates why UCT shows 145 pts vs batch's ~82 pts

use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::prelude::IndexedRandom;
use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::mcts::algorithm::{
    mcts_find_best_position_for_tile_with_nn,
    mcts_find_best_position_for_tile_uct,
};
use take_it_easy::mcts::hyperparameters::MCTSHyperparameters;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::scoring::scoring::result;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔬 Batch MCTS vs UCT MCTS Comparison\n");
    println!("Goal: Investigate why UCT shows 145 pts vs batch's ~82 pts\n");

    // Load neural network
    println!("📂 Loading neural network...");
    let neural_config = NeuralConfig {
        input_dim: (8, 5, 5),
        nn_architecture: NNArchitecture::CNN,
        ..Default::default()
    };
    let mut manager = NeuralManager::with_config(neural_config)?;
    println!("   ✅ Network loaded\n");

    let num_games = 50;  // Smaller for faster comparison
    let simulations = 150;
    let hyperparams = MCTSHyperparameters::default();

    let mut batch_scores: Vec<i32> = Vec::new();
    let mut uct_scores: Vec<i32> = Vec::new();

    println!("🎮 Running {} games with BOTH algorithms (same seed)...\n", num_games);

    for game_idx in 0..num_games {
        // Use same seed for both algorithms
        let seed = 2025 + game_idx as u64;

        // ===== BATCH MCTS =====
        let mut rng_batch = StdRng::seed_from_u64(seed);
        let mut plateau_batch = create_plateau_empty();
        let mut deck_batch = create_deck();

        for turn in 0..19 {
            let available = get_available_tiles(&deck_batch);
            if available.is_empty() {
                break;
            }

            let chosen_tile = *available.choose(&mut rng_batch).unwrap();
            deck_batch = replace_tile_in_deck(&deck_batch, &chosen_tile);

            let policy_net = manager.policy_net();
            let value_net = manager.value_net();

            let mcts_result = mcts_find_best_position_for_tile_with_nn(
                &mut plateau_batch,
                &mut deck_batch,
                chosen_tile,
                policy_net,
                value_net,
                simulations,
                turn,
                19,
                Some(&hyperparams),
            );

            plateau_batch.tiles[mcts_result.best_position] = chosen_tile;
        }

        let batch_score = result(&plateau_batch);
        batch_scores.push(batch_score);

        // ===== UCT MCTS =====
        let mut rng_uct = StdRng::seed_from_u64(seed);
        let mut plateau_uct = create_plateau_empty();
        let mut deck_uct = create_deck();

        for turn in 0..19 {
            let available = get_available_tiles(&deck_uct);
            if available.is_empty() {
                break;
            }

            let chosen_tile = *available.choose(&mut rng_uct).unwrap();
            deck_uct = replace_tile_in_deck(&deck_uct, &chosen_tile);

            let policy_net = manager.policy_net();
            let value_net = manager.value_net();

            let mcts_result = mcts_find_best_position_for_tile_uct(
                &mut plateau_uct,
                &mut deck_uct,
                chosen_tile,
                policy_net,
                value_net,
                simulations,
                turn,
                19,
                Some(&hyperparams),
            );

            plateau_uct.tiles[mcts_result.best_position] = chosen_tile;
        }

        let uct_score = result(&plateau_uct);
        uct_scores.push(uct_score);

        // Show comparison for each game
        if (game_idx + 1) % 10 == 0 {
            println!("  Game {}/{}: Batch={:3} pts, UCT={:3} pts, Diff={:+3}",
                     game_idx + 1, num_games, batch_score, uct_score, uct_score - batch_score);
        }
    }

    // Calculate statistics
    let batch_mean = batch_scores.iter().sum::<i32>() as f64 / batch_scores.len() as f64;
    let batch_std = (batch_scores.iter()
        .map(|&s| (s as f64 - batch_mean).powi(2))
        .sum::<f64>() / batch_scores.len() as f64)
        .sqrt();

    let uct_mean = uct_scores.iter().sum::<i32>() as f64 / uct_scores.len() as f64;
    let uct_std = (uct_scores.iter()
        .map(|&s| (s as f64 - uct_mean).powi(2))
        .sum::<f64>() / uct_scores.len() as f64)
        .sqrt();

    println!("\n{}", "=".repeat(60));
    println!("RESULTS");
    println!("{}", "=".repeat(60));
    
    println!("\n📊 Batch MCTS (Current):");
    println!("  Mean: {:.2} ± {:.2} pts", batch_mean, batch_std);
    println!("  Min/Max: {} / {} pts", 
             batch_scores.iter().min().unwrap(),
             batch_scores.iter().max().unwrap());

    println!("\n📊 UCT MCTS (New):");
    println!("  Mean: {:.2} ± {:.2} pts", uct_mean, uct_std);
    println!("  Min/Max: {} / {} pts",
             uct_scores.iter().min().unwrap(),
             uct_scores.iter().max().unwrap());

    println!("\n📈 Comparison:");
    let diff_mean = uct_mean - batch_mean;
    let diff_pct = (diff_mean / batch_mean) * 100.0;
    println!("  Difference: {:+.2} pts ({:+.1}%)", diff_mean, diff_pct);

    if diff_mean.abs() < 5.0 {
        println!("\n✅ Both algorithms perform similarly (diff < 5 pts)");
        println!("   UCT's 145 pts was likely a statistical anomaly or different config");
    } else if diff_mean > 20.0 {
        println!("\n⚠️  UCT significantly outperforms! (diff > 20 pts)");
        println!("   This suggests UCT is genuinely better OR there's a bug");
    } else {
        println!("\n📝 Moderate difference ({:.1} pts)", diff_mean);
        println!("   Could be real improvement or variance");
    }

    // Detailed game-by-game comparison
    println!("\n📋 Games where algorithms differed significantly (>30 pts):");
    let mut big_diffs = 0;
    for (idx, (&batch, &uct)) in batch_scores.iter().zip(uct_scores.iter()).enumerate() {
        let diff = (uct - batch).abs();
        if diff > 30 {
            println!("  Game {}: Batch={} pts, UCT={} pts, Diff={:+}", 
                     idx + 1, batch, uct, uct - batch);
            big_diffs += 1;
        }
    }
    
    if big_diffs == 0 {
        println!("  (None - both algorithms very consistent)");
    }

    Ok(())
}
