//! Simple GNN Benchmark - Test trained GNN without modifying weights
//!
//! Plays games using the trained GNN with MCTS and reports scores

use clap::Parser;
use flexi_logger::Logger;
use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile;
use take_it_easy::mcts::hyperparameters::MCTSHyperparameters;
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(
    name = "test-gnn-benchmark",
    about = "Benchmark trained GNN performance"
)]
struct Args {
    /// Number of games to play
    #[arg(short, long, default_value_t = 20)]
    games: usize,

    /// Number of MCTS simulations per move
    #[arg(short, long, default_value_t = 150)]
    simulations: usize,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    let args = Args::parse();

    log::info!("🎯 GNN Benchmark Test");
    log::info!("Games: {}", args.games);
    log::info!("MCTS simulations: {}", args.simulations);
    log::info!("Seed: {}", args.seed);

    // Load trained GNN (read-only, won't modify weights)
    log::info!("\n📂 Loading trained GNN...");
    let neural_config = NeuralConfig {
        input_dim: (8, 5, 5),
        nn_architecture: NNArchitecture::Gnn,
        policy_lr: 0.001,
        value_lr: 0.0001,
        ..Default::default()
    };
    let manager = NeuralManager::with_config(neural_config)?;
    log::info!("✅ GNN loaded successfully");

    let hyperparams = MCTSHyperparameters::default();
    let mut rng = StdRng::seed_from_u64(args.seed);

    let mut scores = Vec::new();
    let turns_per_game = 19;

    log::info!("\n🎮 Playing {} games...", args.games);

    for game_id in 0..args.games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..turns_per_game {
            let available_tiles = get_available_tiles(&deck);
            if available_tiles.is_empty() {
                break;
            }

            // Random tile selection
            let chosen_tile = available_tiles[rng.gen_range(0..available_tiles.len())];

            // Use GNN-guided MCTS
            let mcts_result = mcts_find_best_position_for_tile(
                &mut plateau,
                &mut deck,
                chosen_tile,
                args.simulations,
                turn,
                turns_per_game,
                &manager,
                Some(&hyperparams),
            );

            // Apply move
            plateau.tiles[mcts_result.best_position] = chosen_tile;
            deck = replace_tile_in_deck(&deck, &chosen_tile);
        }

        let final_score = result(&plateau);
        scores.push(final_score);

        if (game_id + 1) % 5 == 0 {
            let avg = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
            log::info!("  Game {}/{}: score={} (avg so far: {:.1})", game_id + 1, args.games, final_score, avg);
        }
    }

    // Calculate statistics
    let avg_score = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
    let min_score = *scores.iter().min().unwrap();
    let max_score = *scores.iter().max().unwrap();

    // Calculate standard deviation
    let variance: f64 = scores.iter()
        .map(|&s| {
            let diff = s as f64 - avg_score;
            diff * diff
        })
        .sum::<f64>() / scores.len() as f64;
    let std_dev = variance.sqrt();

    log::info!("\n📊 Benchmark Results:");
    log::info!("  Average score: {:.2} ± {:.2}", avg_score, std_dev);
    log::info!("  Range: [{}, {}]", min_score, max_score);
    log::info!("  Games played: {}", args.games);

    Ok(())
}
