/// Benchmark Random Player (Baseline)
///
/// Plays games with completely random move selection to establish
/// a baseline for comparison with MCTS performance.
///
/// This helps answer: "How much does MCTS actually improve over random play?"
use chrono::Utc;
use clap::Parser;
use flexi_logger::Logger;
use rand::prelude::IndexedRandom;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::error::Error;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(
    name = "benchmark-random-player",
    about = "Benchmark random move selection as baseline for MCTS comparison"
)]
struct Args {
    /// Number of games to simulate
    #[arg(short, long, default_value_t = 100)]
    games: usize,

    /// RNG seed for reproducible results
    #[arg(short = 'r', long, default_value_t = 2025)]
    seed: u64,

    /// Number of turns to play (default 19 for full game)
    #[arg(long, default_value_t = 19)]
    turns: usize,

    /// CSV path to append results
    #[arg(long, default_value = "benchmark_random_player.csv")]
    log_path: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    log::info!("🎲 Random Player Baseline Benchmark");
    log::info!(
        "   Games: {}, Seed: {}, Turns: {}",
        args.games,
        args.seed,
        args.turns
    );

    if args.turns == 0 || args.turns > 27 {
        return Err(format!("Turns must be between 1 and 27 (received {}).", args.turns).into());
    }

    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut scores = Vec::with_capacity(args.games);

    for game_idx in 0..args.games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        // Sample tile sequence for this game
        let mut tile_order = Vec::with_capacity(args.turns);
        for _ in 0..args.turns {
            let available = get_available_tiles(&deck);
            if available.is_empty() {
                break;
            }
            let tile = *available
                .choose(&mut rng)
                .expect("Deck should not be empty");
            tile_order.push(tile);
            deck = replace_tile_in_deck(&deck, &tile);
        }

        // Reset deck and play game with random moves
        deck = create_deck();

        for (turn_idx, &chosen_tile) in tile_order.iter().enumerate() {
            let available_tiles = get_available_tiles(&deck);
            if available_tiles.is_empty() {
                break;
            }

            if !available_tiles.contains(&chosen_tile) {
                log::warn!(
                    "Game {} Turn {}: Tile {:?} not available, skipping.",
                    game_idx + 1,
                    turn_idx + 1,
                    chosen_tile
                );
                break;
            }

            // RANDOM MOVE SELECTION (no MCTS)
            let legal_moves = get_legal_moves(&plateau);
            if legal_moves.is_empty() {
                log::warn!(
                    "Game {} Turn {}: No legal moves available.",
                    game_idx + 1,
                    turn_idx + 1
                );
                break;
            }

            let random_position = *legal_moves
                .choose(&mut rng)
                .expect("Legal moves should not be empty");

            // Play the move
            plateau.tiles[random_position] = chosen_tile;
            deck = replace_tile_in_deck(&deck, &chosen_tile);
        }

        let final_score = result(&plateau);
        scores.push(final_score);

        if (game_idx + 1) % 10 == 0 {
            log::info!("   Completed {}/{} games", game_idx + 1, args.games);
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
    let min_score = *scores.iter().min().unwrap();
    let max_score = *scores.iter().max().unwrap();

    // Print results
    println!("\n{}", "=".repeat(60));
    println!("===== Random Player Baseline =====");
    println!("Games simulated    : {}", args.games);
    println!("Turns per game     : {}", args.turns);
    println!();
    println!(
        "Score              : mean = {:>6.2}, std = {:>6.2}, min = {:>4}, max = {:>4}",
        mean, std_dev, min_score, max_score
    );
    println!();
    println!("Comparison:");
    println!("  MCTS baseline    : ~85 pts ± 28 (from diagnostic)");
    println!("  Random baseline  : {:.2} pts ± {:.2}", mean, std_dev);
    if mean < 85.0 {
        println!(
            "  MCTS improvement : +{:.2} pts (+{:.1}%)",
            85.0 - mean,
            ((85.0 - mean) / mean) * 100.0
        );
    }
    println!("{}", "=".repeat(60));

    // Log to CSV
    if !args.log_path.is_empty() {
        let path = Path::new(&args.log_path);
        let needs_header = !path.exists();

        let mut file = OpenOptions::new().create(true).append(true).open(path)?;

        if needs_header {
            writeln!(file, "timestamp,games,turns,seed,mean,std_dev,min,max")?;
        }

        writeln!(
            file,
            "{},{},{},{},{:.2},{:.2},{},{}",
            Utc::now().format("%Y-%m-%d %H:%M:%S"),
            args.games,
            args.turns,
            args.seed,
            mean,
            std_dev,
            min_score,
            max_score
        )?;

        log::info!("✅ Results appended to {}", args.log_path);
    }

    Ok(())
}
