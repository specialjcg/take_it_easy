//! Benchmark Baseline: Test CNN alone without MCTS optimizations
//!
//! This benchmark tests the neural network in isolation to determine if the
//! regression is due to MCTS optimizations or the network itself.
//!
//! Configuration:
//! - c_puct = 1.41 (classic √2, no adaptation)
//! - No pruning
//! - No rollouts (CNN only)
//! - No temperature annealing
//! - No progressive widening (explore all moves)
//! - weight_cnn = 1.0, all others = 0.0

use clap::Parser;
use flexi_logger::Logger;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::prelude::IndexedRandom;
use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use take_it_easy::mcts::hyperparameters::MCTSHyperparameters;
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "benchmark-baseline-cnn")]
#[command(about = "Benchmark neural network baseline (no MCTS optimizations)")]
struct Args {
    /// Number of games to simulate
    #[arg(short, long, default_value_t = 20)]
    games: usize,

    /// Number of MCTS simulations per move
    #[arg(short, long, default_value_t = 150)]
    simulations: usize,

    /// Random seed for reproducibility
    #[arg(long, default_value_t = 2025)]
    seed: u64,

    /// Number of turns per game
    #[arg(short, long, default_value_t = 19)]
    turns: usize,

    /// Neural network architecture
    #[arg(long, default_value = "CNN")]
    nn_architecture: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    let args = Args::parse();

    log::info!("🧪 Baseline CNN Benchmark (No MCTS Optimizations)");
    log::info!("   Games: {}, Simulations: {}, Seed: {}, Turns: {}",
        args.games, args.simulations, args.seed, args.turns);

    // Parse architecture
    let nn_arch = match args.nn_architecture.to_uppercase().as_str() {
        "CNN" => NNArchitecture::CNN,
        "GNN" => NNArchitecture::GNN,
        _ => return Err(format!("Invalid architecture: {}", args.nn_architecture).into()),
    };

    // Initialize neural network
    let neural_config = NeuralConfig {
        input_dim: (8, 5, 5),
        nn_architecture: nn_arch,
        ..Default::default()
    };
    let manager = NeuralManager::with_config(neural_config)?;

    // Create baseline hyperparameters (CNN-only, no optimizations)
    let hyperparams = MCTSHyperparameters {
        // Classic UCT constant (√2), no adaptation
        c_puct_early: 1.41,
        c_puct_mid: 1.41,
        c_puct_late: 1.41,
        variance_mult_high: 1.0,
        variance_mult_low: 1.0,

        // No pruning - explore all legal moves
        prune_early: 0.0,
        prune_mid1: 0.0,
        prune_mid2: 0.0,
        prune_late: 0.0,

        // No rollouts - CNN only
        rollout_strong: 0,
        rollout_medium: 0,
        rollout_default: 0,
        rollout_weak: 0,

        // CNN only, no other evaluators
        weight_cnn: 1.0,
        weight_rollout: 0.0,
        weight_heuristic: 0.0,
        weight_contextual: 0.0,

        // No adaptive simulations
        sim_mult_early: 1.0,
        sim_mult_mid: 1.0,
        sim_mult_late: 1.0,

        // No temperature annealing
        temp_initial: 1.0,
        temp_final: 1.0,
        temp_decay_start: 100,
        temp_decay_end: 100,

        // RAVE disabled
        rave_k: 0.0,
    };

    log::info!("📊 Baseline Configuration:");
    log::info!("   c_puct: 1.41 (constant)");
    log::info!("   Pruning: disabled");
    log::info!("   Rollouts: 0 (CNN only)");
    log::info!("   Weight CNN: 1.0 (100%)");
    log::info!("   Temperature: 1.0 (constant)");
    log::info!("   Progressive Widening: will explore ALL legal moves");

    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut scores = Vec::new();

    for game_idx in 0..args.games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..args.turns {
            let available = get_available_tiles(&deck);
            if available.is_empty() {
                break;
            }

            let chosen_tile = *available.choose(&mut rng).unwrap();
            deck = replace_tile_in_deck(&deck, &chosen_tile);

            let mcts_result = mcts_find_best_position_for_tile_with_nn(
                &mut plateau,
                &mut deck,
                chosen_tile,
                manager.policy_net(),
                manager.value_net(),
                args.simulations,
                turn,
                args.turns,
                Some(&hyperparams),
            );

            plateau.tiles[mcts_result.best_position] = chosen_tile;
        }

        let final_score = result(&plateau);
        scores.push(final_score);

        if (game_idx + 1) % 10 == 0 {
            log::info!("  Completed {}/{} games", game_idx + 1, args.games);
        }
    }

    // Calculate statistics
    let mean = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
    let variance = scores
        .iter()
        .map(|&s| {
            let diff = s as f64 - mean;
            diff * diff
        })
        .sum::<f64>()
        / scores.len() as f64;
    let std_dev = variance.sqrt();
    let min = *scores.iter().min().unwrap();
    let max = *scores.iter().max().unwrap();

    println!("\n===== Baseline CNN Benchmark Results =====");
    println!("Games simulated    : {}", args.games);
    println!("Simulations/move   : {}", args.simulations);
    println!("Turns per game     : {}", args.turns);
    println!();
    println!("Score              : mean = {:.2}, std = {:.2}, min = {:4}, max = {:4}",
        mean, std_dev, min, max);
    println!();
    println!("Configuration      : CNN-only (no optimizations)");
    println!("Expected (if NN good): > 120 pts");
    println!("Expected (if NN bad) : < 100 pts");
    println!("===========================================");

    Ok(())
}
