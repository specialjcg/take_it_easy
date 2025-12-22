use chrono::Utc;
use clap::Parser;
use flexi_logger::Logger;
use rand::prelude::IndexedRandom;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::error::Error;
use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::mcts::algorithm::{
    mcts_find_best_position_for_tile_gumbel, mcts_find_best_position_for_tile_with_nn,
};
use take_it_easy::neural::policy_value_net::{PolicyNet, ValueNet};
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use take_it_easy::scoring::scoring::result;

#[derive(clap::ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum NnArchitectureCli {
    Cnn,
    Gnn,
}

impl From<NnArchitectureCli> for take_it_easy::neural::manager::NNArchitecture {
    fn from(cli: NnArchitectureCli) -> Self {
        match cli {
            NnArchitectureCli::Cnn => take_it_easy::neural::manager::NNArchitecture::CNN,
            NnArchitectureCli::Gnn => take_it_easy::neural::manager::NNArchitecture::GNN,
        }
    }
}

impl fmt::Display for NnArchitectureCli {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NnArchitectureCli::Cnn => write!(f, "cnn"),
            NnArchitectureCli::Gnn => write!(f, "gnn"),
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "test-gumbel",
    about = "Test Gumbel MCTS against standard MCTS baseline."
)]
struct Args {
    /// Number of self-play games to simulate
    #[arg(short, long, default_value_t = 10)]
    games: usize,

    /// Number of MCTS simulations per move
    #[arg(short, long, default_value_t = 150)]
    simulations: usize,

    /// RNG seed
    #[arg(short = 'r', long, default_value_t = 2025)]
    seed: u64,

    /// Number of turns to play (default 19 for full game)
    #[arg(long, default_value_t = 19)]
    turns: usize,

    /// CSV path to append benchmark results
    #[arg(long, default_value = "gumbel_mcts_log.csv")]
    log_path: String,

    /// Neural network architecture (cnn or gnn)
    #[arg(long, value_enum, default_value = "cnn")]
    nn_architecture: NnArchitectureCli,

    /// Use Gumbel MCTS (if false, uses baseline)
    #[arg(long, default_value_t = false)]
    use_gumbel: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    let approach = if args.use_gumbel {
        "Gumbel MCTS"
    } else {
        "Baseline MCTS"
    };

    log::info!(
        "🎮 Testing {}: games={}, simulations={}, seed={}, turns={}",
        approach,
        args.games,
        args.simulations,
        args.seed,
        args.turns
    );

    if args.turns == 0 || args.turns > 27 {
        return Err(format!("Turns must be between 1 and 27 (received {}).", args.turns).into());
    }

    let neural_config = NeuralConfig {
        input_dim: (8, 5, 5),
        nn_architecture: args.nn_architecture.clone().into(),
        ..Default::default()
    };

    let manager = NeuralManager::with_config(neural_config)?;
    let policy_net: &PolicyNet = manager.policy_net();
    let value_net: &ValueNet = manager.value_net();

    log::info!("✅ Neural networks loaded: {:?}", args.nn_architecture);

    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut all_scores = Vec::new();

    for game_idx in 0..args.games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        let available = get_available_tiles(&deck);
        let chosen_tiles: Vec<Tile> = available
            .sample(&mut rng, args.turns.min(available.len()))
            .copied()
            .collect();

        for (turn_idx, &chosen_tile) in chosen_tiles.iter().enumerate() {
            let available_tiles = get_available_tiles(&deck);
            if available_tiles.is_empty() {
                break;
            }

            if !available_tiles.contains(&chosen_tile) {
                log::warn!(
                    "Game {} Turn {}: Tile {:?} not available in deck, skipping.",
                    game_idx + 1,
                    turn_idx + 1,
                    chosen_tile
                );
                break;
            }

            let mcts_result = if args.use_gumbel {
                mcts_find_best_position_for_tile_gumbel(
                    &mut plateau,
                    &mut deck,
                    chosen_tile,
                    &policy_net,
                    &value_net,
                    args.simulations,
                    turn_idx,
                    args.turns,
                    None,
                )
            } else {
                mcts_find_best_position_for_tile_with_nn(
                    &mut plateau,
                    &mut deck,
                    chosen_tile,
                    &policy_net,
                    &value_net,
                    args.simulations,
                    turn_idx,
                    args.turns,
                    None,
                )
            };

            let best_position = mcts_result.best_position;
            plateau.tiles[best_position] = chosen_tile;
            deck = replace_tile_in_deck(&deck, &chosen_tile);
        }

        let final_score = result(&plateau);
        all_scores.push(final_score);

        log::info!(
            "Game {:3}/{:3} | {} | Final Score: {}",
            game_idx + 1,
            args.games,
            approach,
            final_score
        );
    }

    let avg_score = all_scores.iter().sum::<i32>() as f64 / all_scores.len() as f64;
    let variance = all_scores
        .iter()
        .map(|&s| {
            let diff = s as f64 - avg_score;
            diff * diff
        })
        .sum::<f64>()
        / all_scores.len() as f64;
    let std_dev = variance.sqrt();

    log::info!("{}", "=".repeat(60));
    log::info!("📊 {} Results:", approach);
    log::info!("   Games: {}", args.games);
    log::info!("   Simulations: {}", args.simulations);
    log::info!("   Average Score: {:.2}", avg_score);
    log::info!("   Std Dev: {:.2}", std_dev);
    log::info!(
        "   Min/Max: {}/{}",
        all_scores.iter().min().unwrap(),
        all_scores.iter().max().unwrap()
    );
    log::info!("{}", "=".repeat(60));

    // Log to CSV
    if !args.log_path.is_empty() {
        let path = Path::new(&args.log_path);
        let needs_header = !path.exists();

        let mut file = OpenOptions::new().create(true).append(true).open(path)?;

        if needs_header {
            writeln!(
                file,
                "timestamp,approach,games,simulations,seed,turns,nn_architecture,avg_score,std_dev,min_score,max_score"
            )?;
        }

        writeln!(
            file,
            "{},{},{},{},{},{},{},{:.2},{:.2},{},{}",
            Utc::now().format("%Y-%m-%d %H:%M:%S"),
            approach,
            args.games,
            args.simulations,
            args.seed,
            args.turns,
            args.nn_architecture,
            avg_score,
            std_dev,
            all_scores.iter().min().unwrap(),
            all_scores.iter().max().unwrap()
        )?;

        log::info!("✅ Results appended to {}", args.log_path);
    }

    Ok(())
}
