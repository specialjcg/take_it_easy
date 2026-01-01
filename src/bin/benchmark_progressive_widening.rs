/// Benchmark Progressive Widening (Sprint 1) vs Baseline
///
/// Compares MCTS performance with and without Progressive Widening enabled.
/// Uses identical tile sequences for fair comparison.
///
/// Expected improvement: +15-25% score with -40% redundant simulations

use chrono::Utc;
use clap::Parser;
use flexi_logger::Logger;
use rand::prelude::IndexedRandom;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(
    name = "benchmark-progressive-widening",
    about = "Benchmark Progressive Widening optimization (Sprint 1)"
)]
struct Args {
    /// Number of self-play games to simulate
    #[arg(short, long, default_value_t = 100)]
    games: usize,

    /// Number of MCTS simulations per move
    #[arg(short, long, default_value_t = 150)]
    simulations: usize,

    /// RNG seed for reproducible tile sequences
    #[arg(short = 'r', long, default_value_t = 2025)]
    seed: u64,

    /// Number of turns to play (default 19 for full game)
    #[arg(long, default_value_t = 19)]
    turns: usize,

    /// CSV path to append benchmark results
    #[arg(long, default_value = "benchmark_progressive_widening.csv")]
    log_path: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    log::info!(
        "🎮 Sprint 1 Benchmark: Progressive Widening vs Baseline"
    );
    log::info!(
        "   Games: {}, Simulations: {}, Seed: {}, Turns: {}",
        args.games,
        args.simulations,
        args.seed,
        args.turns
    );

    if args.turns == 0 || args.turns > 27 {
        return Err(format!("Turns must be between 1 and 27 (received {}).", args.turns).into());
    }

    // Load neural networks (same for both variants)
    let neural_config = NeuralConfig {
        input_dim: (9, 5, 5),
        nn_architecture: NNArchitecture::Cnn,
        ..Default::default()
    };
    let manager = NeuralManager::with_config(neural_config)?;

    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut scores = Vec::with_capacity(args.games);

    // Run benchmark with Progressive Widening (current code)
    for game_index in 0..args.games {
        let tile_order = sample_tile_sequence(&mut rng, args.turns);
        if tile_order.len() < args.turns {
            log::warn!(
                "Game {} terminated early: only {} tiles generated.",
                game_index,
                tile_order.len()
            );
        }

        let policy_net = manager.policy_net();
        let value_net = manager.value_net();

        let score = play_game(
            &tile_order,
            args.simulations,
            policy_net,
            value_net,
        );

        scores.push(score);

        if (game_index + 1) % 10 == 0 {
            log::info!("  Completed {}/{} games", game_index + 1, args.games);
        }
    }

    let stats = compute_stats(&scores);

    println!("\n===== Progressive Widening Benchmark (Sprint 1) =====");
    println!("Games simulated    : {}", args.games);
    println!("Simulations/move   : {}", args.simulations);
    println!("Turns per game     : {}", args.turns);
    println!();
    println!(
        "Score              : mean = {:>6.2}, std = {:>6.2}, min = {:>4}, max = {:>4}",
        stats.mean, stats.std_dev, stats.min, stats.max
    );
    println!();
    println!("Baseline reference : 159.95 pts (from hyperparameters.rs)");
    println!("Improvement        : {:+.2} pts ({:+.1}%)",
        stats.mean - 159.95,
        ((stats.mean - 159.95) / 159.95) * 100.0
    );
    println!("Expected range     : +15-25% (+24-40 pts)");
    println!("===================================================\n");

    if !args.log_path.trim().is_empty() {
        if let Err(e) = append_log(
            &args.log_path,
            &args,
            &stats,
        ) {
            eprintln!("[benchmark_progressive_widening] failed to append log: {}", e);
        }
    }

    Ok(())
}

fn play_game(
    tiles: &[Tile],
    num_simulations: usize,
    policy_net: &take_it_easy::neural::policy_value_net::PolicyNet,
    value_net: &take_it_easy::neural::policy_value_net::ValueNet,
) -> i32 {
    let mut deck = create_deck();
    let mut plateau = create_plateau_empty();
    let total_turns = tiles.len();

    for (turn, tile) in tiles.iter().enumerate() {
        let mcts_result = mcts_find_best_position_for_tile_with_nn(
            &mut plateau,
            &mut deck,
            *tile,
            policy_net,
            value_net,
            num_simulations,
            turn,
            total_turns,
            None, // Use default hyperparameters (with Progressive Widening)
        );

        plateau.tiles[mcts_result.best_position] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn sample_tile_sequence(rng: &mut StdRng, turns: usize) -> Vec<Tile> {
    let mut deck = create_deck();
    let mut sequence = Vec::with_capacity(turns);

    for _ in 0..turns {
        let available = get_available_tiles(&deck);
        if available.is_empty() {
            break;
        }

        let tile = *available.choose(rng).expect("Deck should not be empty");
        sequence.push(tile);
        deck = replace_tile_in_deck(&deck, &tile);
    }

    sequence
}

struct Stats {
    mean: f64,
    std_dev: f64,
    min: i32,
    max: i32,
}

fn compute_stats(scores: &[i32]) -> Stats {
    if scores.is_empty() {
        return Stats {
            mean: 0.0,
            std_dev: 0.0,
            min: 0,
            max: 0,
        };
    }

    let mean = scores.iter().map(|&s| s as f64).sum::<f64>() / scores.len() as f64;
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

    Stats {
        mean,
        std_dev,
        min,
        max,
    }
}

fn append_log(
    path: &str,
    args: &Args,
    stats: &Stats,
) -> Result<(), String> {
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("create log dir failed: {e}"))?;
        }
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("open log failed: {e}"))?;

    if file
        .metadata()
        .map_err(|e| format!("metadata failed: {e}"))?
        .len()
        == 0
    {
        writeln!(
            file,
            "timestamp,games,simulations,mean,std_dev,min,max,baseline_delta,improvement_pct,turns,seed,sprint"
        )
        .map_err(|e| format!("write header failed: {e}"))?;
    }

    let timestamp = Utc::now().to_rfc3339();
    let baseline = 159.95;
    let delta = stats.mean - baseline;
    let improvement_pct = (delta / baseline) * 100.0;

    writeln!(
        file,
        "{},{},{},{:.2},{:.2},{},{},{:.2},{:.2},{},{},Sprint1-ProgressiveWidening",
        timestamp,
        args.games,
        args.simulations,
        stats.mean,
        stats.std_dev,
        stats.min,
        stats.max,
        delta,
        improvement_pct,
        args.turns,
        args.seed,
    )
    .map_err(|e| format!("write row failed: {e}"))?;

    Ok(())
}
