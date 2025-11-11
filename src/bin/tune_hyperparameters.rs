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
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use take_it_easy::mcts::hyperparameters::MCTSHyperparameters;
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
    name = "tune-hyperparameters",
    about = "Tune MCTS hyperparameters to optimize performance."
)]
struct Args {
    /// Number of self-play games to simulate
    #[arg(short, long, default_value_t = 20)]
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
    #[arg(long, default_value = "hyperparameter_tuning_log.csv")]
    log_path: String,

    /// Neural network architecture (cnn or gnn)
    #[arg(long, value_enum, default_value = "cnn")]
    nn_architecture: NnArchitectureCli,

    // === c_puct parameters ===
    /// c_puct value for early game (turns 0-4)
    #[arg(long, default_value_t = 4.2)]
    c_puct_early: f64,

    /// c_puct value for mid game (turns 5-15)
    #[arg(long, default_value_t = 3.8)]
    c_puct_mid: f64,

    /// c_puct value for late game (turns 16+)
    #[arg(long, default_value_t = 3.0)]
    c_puct_late: f64,

    /// Variance multiplier for high variance (>0.5)
    #[arg(long, default_value_t = 1.3)]
    variance_mult_high: f64,

    /// Variance multiplier for low variance (<0.05)
    #[arg(long, default_value_t = 0.85)]
    variance_mult_low: f64,

    // === Pruning parameters ===
    /// Pruning ratio for early game (turns 0-4)
    #[arg(long, default_value_t = 0.05)]
    prune_early: f64,

    /// Pruning ratio for mid-early game (turns 5-9)
    #[arg(long, default_value_t = 0.10)]
    prune_mid1: f64,

    /// Pruning ratio for mid-late game (turns 10-14)
    #[arg(long, default_value_t = 0.15)]
    prune_mid2: f64,

    /// Pruning ratio for late game (turns 15+)
    #[arg(long, default_value_t = 0.20)]
    prune_late: f64,

    // === Rollout count parameters ===
    /// Rollout count for strong positions (value_estimate > 0.7)
    #[arg(long, default_value_t = 3)]
    rollout_strong: usize,

    /// Rollout count for medium positions (value_estimate > 0.2)
    #[arg(long, default_value_t = 5)]
    rollout_medium: usize,

    /// Rollout count for default positions
    #[arg(long, default_value_t = 7)]
    rollout_default: usize,

    /// Rollout count for weak positions (value_estimate < -0.4)
    #[arg(long, default_value_t = 9)]
    rollout_weak: usize,

    // === Evaluation weight parameters ===
    /// Weight for CNN value prediction
    #[arg(long, default_value_t = 0.6)]
    weight_cnn: f64,

    /// Weight for pattern rollout evaluation
    #[arg(long, default_value_t = 0.2)]
    weight_rollout: f64,

    /// Weight for heuristic evaluation
    #[arg(long, default_value_t = 0.1)]
    weight_heuristic: f64,

    /// Weight for contextual evaluation
    #[arg(long, default_value_t = 0.1)]
    weight_contextual: f64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    log::info!(
        "🎯 Hyperparameter Tuning: games={}, simulations={}, seed={}, turns={}",
        args.games,
        args.simulations,
        args.seed,
        args.turns
    );

    if args.turns == 0 || args.turns > 27 {
        return Err(format!("Turns must be between 1 and 27 (received {}).", args.turns).into());
    }

    // Create hyperparameters from CLI args
    let hyperparams = MCTSHyperparameters {
        c_puct_early: args.c_puct_early,
        c_puct_mid: args.c_puct_mid,
        c_puct_late: args.c_puct_late,
        variance_mult_high: args.variance_mult_high,
        variance_mult_low: args.variance_mult_low,
        prune_early: args.prune_early,
        prune_mid1: args.prune_mid1,
        prune_mid2: args.prune_mid2,
        prune_late: args.prune_late,
        rollout_strong: args.rollout_strong,
        rollout_medium: args.rollout_medium,
        rollout_default: args.rollout_default,
        rollout_weak: args.rollout_weak,
        weight_cnn: args.weight_cnn,
        weight_rollout: args.weight_rollout,
        weight_heuristic: args.weight_heuristic,
        weight_contextual: args.weight_contextual,
        // Quick Wins defaults (not tunable via CLI in this binary)
        sim_mult_early: 0.67,
        sim_mult_mid: 1.0,
        sim_mult_late: 1.67,
        temp_initial: 1.8,
        temp_final: 0.5,
        temp_decay_start: 7,
        temp_decay_end: 13,
    };

    // Validate hyperparameters
    if let Err(e) = hyperparams.validate_weights() {
        return Err(format!("Invalid hyperparameters: {}", e).into());
    }

    log::info!("📋 Hyperparameter Configuration:");
    log::info!(
        "   c_puct: {:.2}/{:.2}/{:.2} (early/mid/late)",
        hyperparams.c_puct_early,
        hyperparams.c_puct_mid,
        hyperparams.c_puct_late
    );
    log::info!(
        "   variance_mult: {:.2}/{:.2} (high/low)",
        hyperparams.variance_mult_high,
        hyperparams.variance_mult_low
    );
    log::info!(
        "   prune_ratio: {:.2}/{:.2}/{:.2}/{:.2} (early/mid1/mid2/late)",
        hyperparams.prune_early,
        hyperparams.prune_mid1,
        hyperparams.prune_mid2,
        hyperparams.prune_late
    );
    log::info!(
        "   rollout_count: {}/{}/{}/{} (strong/medium/default/weak)",
        hyperparams.rollout_strong,
        hyperparams.rollout_medium,
        hyperparams.rollout_default,
        hyperparams.rollout_weak
    );
    log::info!(
        "   weights: CNN={:.2}, Rollout={:.2}, Heuristic={:.2}, Contextual={:.2}",
        hyperparams.weight_cnn,
        hyperparams.weight_rollout,
        hyperparams.weight_heuristic,
        hyperparams.weight_contextual
    );

    let neural_config = NeuralConfig {
        input_dim: (8, 5, 5),
        nn_architecture: args.nn_architecture.clone().into(),
        ..Default::default()
    };

    let manager = NeuralManager::with_config(neural_config)?;
    log::info!("✅ Neural networks loaded: {:?}", args.nn_architecture);

    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut all_scores = Vec::new();

    for game_idx in 0..args.games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        // Sample tile sequence for this game
        let mut chosen_tiles = Vec::with_capacity(args.turns);
        for _ in 0..args.turns {
            let available = get_available_tiles(&deck);
            if available.is_empty() {
                break;
            }
            let tile = *available
                .choose(&mut rng)
                .expect("Deck should not be empty");
            chosen_tiles.push(tile);
            deck = replace_tile_in_deck(&deck, &tile);
        }

        // Reset deck for actual gameplay
        deck = create_deck();

        // Get neural networks (borrowed)
        let policy_net = manager.policy_net();
        let value_net = manager.value_net();

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

            let mcts_result = mcts_find_best_position_for_tile_with_nn(
                &mut plateau,
                &mut deck,
                chosen_tile,
                &policy_net,
                &value_net,
                args.simulations,
                turn_idx,
                args.turns,
                Some(&hyperparams), // Use custom hyperparameters
            );

            let best_position = mcts_result.best_position;
            plateau.tiles[best_position] = chosen_tile;
            deck = replace_tile_in_deck(&deck, &chosen_tile);
        }

        let final_score = result(&plateau);
        all_scores.push(final_score);

        log::info!(
            "Game {:3}/{:3} | Final Score: {}",
            game_idx + 1,
            args.games,
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
    log::info!("📊 Tuning Results:");
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
                "timestamp,games,simulations,seed,turns,nn_architecture,\
                 c_puct_early,c_puct_mid,c_puct_late,variance_mult_high,variance_mult_low,\
                 prune_early,prune_mid1,prune_mid2,prune_late,\
                 rollout_strong,rollout_medium,rollout_default,rollout_weak,\
                 weight_cnn,weight_rollout,weight_heuristic,weight_contextual,\
                 avg_score,std_dev,min_score,max_score"
            )?;
        }

        writeln!(
            file,
            "{},{},{},{},{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{},{},{:.3},{:.3},{:.3},{:.3},{:.2},{:.2},{},{}",
            Utc::now().format("%Y-%m-%d %H:%M:%S"),
            args.games,
            args.simulations,
            args.seed,
            args.turns,
            args.nn_architecture,
            hyperparams.c_puct_early,
            hyperparams.c_puct_mid,
            hyperparams.c_puct_late,
            hyperparams.variance_mult_high,
            hyperparams.variance_mult_low,
            hyperparams.prune_early,
            hyperparams.prune_mid1,
            hyperparams.prune_mid2,
            hyperparams.prune_late,
            hyperparams.rollout_strong,
            hyperparams.rollout_medium,
            hyperparams.rollout_default,
            hyperparams.rollout_weak,
            hyperparams.weight_cnn,
            hyperparams.weight_rollout,
            hyperparams.weight_heuristic,
            hyperparams.weight_contextual,
            avg_score,
            std_dev,
            all_scores.iter().min().unwrap(),
            all_scores.iter().max().unwrap()
        )?;

        log::info!("✅ Results appended to {}", args.log_path);
    }

    Ok(())
}
