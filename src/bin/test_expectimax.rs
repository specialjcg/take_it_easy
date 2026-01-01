//! Test binary for Expectimax MCTS
//!
//! Simple benchmark to test the new Expectimax MCTS algorithm
//! and compare it with the baseline Pattern Rollouts V2.

use clap::Parser;
use flexi_logger::Logger;
use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::error::Error;
use std::time::Instant;
use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use take_it_easy::mcts::expectimax_algorithm::expectimax_mcts_find_best_position;
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "test_expectimax")]
#[command(about = "Test Expectimax MCTS vs baseline")]
struct Args {
    /// Number of games to play
    #[arg(short, long, default_value_t = 10)]
    games: usize,

    /// Number of MCTS simulations per move
    #[arg(short, long, default_value_t = 150)]
    simulations: usize,

    /// RNG seed
    #[arg(long, default_value_t = 2025)]
    seed: u64,

    /// Use Expectimax MCTS (otherwise uses baseline)
    #[arg(long, default_value_t = false)]
    use_expectimax: bool,

    /// Neural network architecture (cnn or gnn)
    #[arg(long, default_value = "cnn")]
    nn_architecture: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    let args = Args::parse();

    let arch = match args.nn_architecture.as_str() {
        "gnn" => NNArchitecture::Gnn,
        _ => NNArchitecture::Cnn,
    };

    println!("🎮 Expectimax MCTS Test");
    println!("========================");
    println!("Games: {}", args.games);
    println!("Simulations per move: {}", args.simulations);
    println!(
        "Algorithm: {}",
        if args.use_expectimax {
            "Expectimax MCTS"
        } else {
            "Baseline (Pattern Rollouts V2)"
        }
    );
    println!("Architecture: {:?}", arch);
    println!("Seed: {}", args.seed);
    println!();

    // Load neural networks
    log::info!("Loading neural networks...");
    let neural_config = NeuralConfig {
        input_dim: (9, 5, 5),
        nn_architecture: arch,
        ..Default::default()
    };
    let manager = NeuralManager::with_config(neural_config)?;
    println!("✅ Neural networks loaded");
    println!();

    // Play games
    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut scores = Vec::new();
    let mut total_time_ms = 0u128;

    for game_num in 0..args.games {
        let start_time = Instant::now();

        let policy_net = manager.policy_net();
        let value_net = manager.value_net();

        let score = play_one_game(
            &mut rng,
            policy_net,
            value_net,
            args.simulations,
            args.use_expectimax,
        );

        let elapsed = start_time.elapsed().as_millis();
        total_time_ms += elapsed;

        scores.push(score);

        println!(
            "Game {}/{}: {} pts ({} ms)",
            game_num + 1,
            args.games,
            score,
            elapsed
        );
    }

    // Statistics
    println!();
    println!("📊 Results");
    println!("==========");

    let avg_score = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
    let min_score = *scores.iter().min().unwrap();
    let max_score = *scores.iter().max().unwrap();
    let avg_time = total_time_ms as f64 / scores.len() as f64;

    // Calculate standard deviation
    let variance = scores
        .iter()
        .map(|&s| {
            let diff = s as f64 - avg_score;
            diff * diff
        })
        .sum::<f64>()
        / scores.len() as f64;
    let std_dev = variance.sqrt();

    println!("Average score: {:.2} pts", avg_score);
    println!("Std deviation: {:.2} pts", std_dev);
    println!("Min score: {} pts", min_score);
    println!("Max score: {} pts", max_score);
    println!("Average time per game: {:.0} ms", avg_time);
    println!("Average time per move: {:.0} ms", avg_time / 19.0);

    // Comparison with baseline (139.40 pts)
    let baseline = 139.40;
    let delta = avg_score - baseline;
    let delta_pct = (delta / baseline) * 100.0;

    println!();
    println!("📈 vs Baseline (Pattern Rollouts V2 = 139.40 pts)");
    println!("====================================================");
    println!("Delta: {:+.2} pts ({:+.1}%)", delta, delta_pct);

    if avg_score >= 143.0 {
        println!("✅ SUCCESS: Target of 143 pts REACHED!");
    } else if avg_score >= baseline {
        println!("✅ IMPROVEMENT: Better than baseline");
    } else {
        println!("❌ REGRESSION: Worse than baseline");
    }

    println!();
    println!("Scores: {:?}", scores);

    Ok(())
}

fn play_one_game(
    rng: &mut StdRng,
    policy_net: &take_it_easy::neural::policy_value_net::PolicyNet,
    value_net: &take_it_easy::neural::policy_value_net::ValueNet,
    num_simulations: usize,
    use_expectimax: bool,
) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    // Play 19 turns
    for turn in 0..19 {
        // Draw a random tile from available tiles
        let available = get_available_tiles(&deck);
        if available.is_empty() {
            break;
        }
        let chosen_tile = *available.choose(rng).expect("Deck should not be empty");

        let mcts_result = if use_expectimax {
            expectimax_mcts_find_best_position(
                &mut plateau,
                &mut deck,
                chosen_tile,
                policy_net,
                value_net,
                num_simulations,
                turn,
                19,
            )
        } else {
            mcts_find_best_position_for_tile_with_nn(
                &mut plateau,
                &mut deck,
                chosen_tile,
                policy_net,
                value_net,
                num_simulations,
                turn,
                19,
                None,
            )
        };

        // Play the move
        let position = mcts_result.best_position;
        plateau.tiles[position] = chosen_tile;
        deck = replace_tile_in_deck(&deck, &chosen_tile);
    }

    result(&plateau)
}
