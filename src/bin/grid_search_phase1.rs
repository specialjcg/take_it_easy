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
    name = "grid-search-phase1",
    about = "Phase 1: Grid search for optimal evaluation weights."
)]
struct Args {
    /// Number of games per configuration
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
}

/// Configuration for one grid search point
#[derive(Debug, Clone)]
struct GridConfig {
    weight_cnn: f64,
    weight_rollout: f64,
    weight_heuristic: f64,
    weight_contextual: f64,
}

impl GridConfig {
    fn to_hyperparameters(&self) -> MCTSHyperparameters {
        MCTSHyperparameters {
            c_puct_early: 4.2,
            c_puct_mid: 3.8,
            c_puct_late: 3.0,
            variance_mult_high: 1.3,
            variance_mult_low: 0.85,
            prune_early: 0.05,
            prune_mid1: 0.10,
            prune_mid2: 0.15,
            prune_late: 0.20,
            rollout_strong: 3,
            rollout_medium: 5,
            rollout_default: 7,
            rollout_weak: 9,
            weight_cnn: self.weight_cnn,
            weight_rollout: self.weight_rollout,
            weight_heuristic: self.weight_heuristic,
            weight_contextual: self.weight_contextual,
            // Quick Wins defaults
            sim_mult_early: 0.67,
            sim_mult_mid: 1.0,
            sim_mult_late: 1.67,
            temp_initial: 1.8,
            temp_final: 0.5,
            temp_decay_start: 7,
            temp_decay_end: 13,
        }
    }
}

/// Generate all valid grid configurations for Phase 1
fn generate_phase1_grid() -> Vec<GridConfig> {
    let cnn_values = [0.55, 0.60, 0.65];
    let rollout_values = [0.15, 0.20, 0.25];
    let heuristic_values = [0.05, 0.10, 0.15];

    let mut configs = Vec::new();

    for &w_cnn in &cnn_values {
        for &w_roll in &rollout_values {
            for &w_heur in &heuristic_values {
                let w_ctx = 1.0 - w_cnn - w_roll - w_heur;

                // Only include if contextual weight is in valid range [0.04, 0.16]
                // (with small tolerance for floating point)
                if w_ctx >= 0.04 && w_ctx <= 0.16 {
                    configs.push(GridConfig {
                        weight_cnn: w_cnn,
                        weight_rollout: w_roll,
                        weight_heuristic: w_heur,
                        weight_contextual: w_ctx,
                    });
                }
            }
        }
    }

    configs
}

/// Run evaluation for one configuration
fn evaluate_config(
    config: &GridConfig,
    args: &Args,
    manager: &NeuralManager,
) -> Result<(f64, f64, i32, i32), Box<dyn Error>> {
    let hyperparams = config.to_hyperparameters();

    // Validate hyperparameters
    hyperparams.validate_weights()?;

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
                    "Config [{:.2}/{:.2}/{:.2}/{:.2}] Game {} Turn {}: Tile not available, skipping.",
                    config.weight_cnn,
                    config.weight_rollout,
                    config.weight_heuristic,
                    config.weight_contextual,
                    game_idx + 1,
                    turn_idx + 1
                );
                break;
            }

            let mcts_result = mcts_find_best_position_for_tile_with_nn(
                &mut plateau,
                &mut deck,
                chosen_tile,
                policy_net,
                value_net,
                args.simulations,
                turn_idx,
                args.turns,
                Some(&hyperparams),
            );

            let best_position = mcts_result.best_position;
            plateau.tiles[best_position] = chosen_tile;
            deck = replace_tile_in_deck(&deck, &chosen_tile);
        }

        let final_score = result(&plateau);
        all_scores.push(final_score);
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
    let min_score = *all_scores.iter().min().unwrap();
    let max_score = *all_scores.iter().max().unwrap();

    Ok((avg_score, std_dev, min_score, max_score))
}

/// Log results to CSV
fn log_to_csv(
    path: &str,
    args: &Args,
    config: &GridConfig,
    avg_score: f64,
    std_dev: f64,
    min_score: i32,
    max_score: i32,
) -> Result<(), Box<dyn Error>> {
    let path = Path::new(path);
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

    let hyperparams = config.to_hyperparameters();

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
        config.weight_cnn,
        config.weight_rollout,
        config.weight_heuristic,
        config.weight_contextual,
        avg_score,
        std_dev,
        min_score,
        max_score
    )?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    log::info!("🔍 Phase 1: Evaluation Weights Grid Search");
    log::info!("============================================");
    log::info!("Games per config: {}", args.games);
    log::info!("Simulations: {}", args.simulations);
    log::info!("Seed: {}", args.seed);
    log::info!("Log file: {}", args.log_path);
    log::info!("");

    if args.turns == 0 || args.turns > 27 {
        return Err(format!("Turns must be between 1 and 27 (received {}).", args.turns).into());
    }

    // Generate grid configurations
    let configs = generate_phase1_grid();
    let total_configs = configs.len();

    log::info!("Total configurations to test: {}", total_configs);
    log::info!("");

    // Initialize neural network
    let neural_config = NeuralConfig {
        input_dim: (8, 5, 5),
        nn_architecture: args.nn_architecture.clone().into(),
        ..Default::default()
    };

    let manager = NeuralManager::with_config(neural_config)?;
    log::info!("✅ Neural networks loaded: {:?}", args.nn_architecture);
    log::info!("");

    // Track best configuration
    let mut best_score = f64::NEG_INFINITY;
    let mut best_config: Option<GridConfig> = None;

    // Evaluate each configuration
    for (idx, config) in configs.iter().enumerate() {
        log::info!(
            "[{}/{}] Testing: CNN={:.2}, Roll={:.2}, Heur={:.2}, Ctx={:.2}",
            idx + 1,
            total_configs,
            config.weight_cnn,
            config.weight_rollout,
            config.weight_heuristic,
            config.weight_contextual
        );

        let (avg_score, std_dev, min_score, max_score) = evaluate_config(config, &args, &manager)?;

        log::info!(
            "   Result: {:.2} ± {:.2} pts (range: {}-{})",
            avg_score,
            std_dev,
            min_score,
            max_score
        );

        // Log to CSV
        if !args.log_path.is_empty() {
            log_to_csv(
                &args.log_path,
                &args,
                config,
                avg_score,
                std_dev,
                min_score,
                max_score,
            )?;
        }

        // Track best
        if avg_score > best_score {
            best_score = avg_score;
            best_config = Some(config.clone());
        }

        log::info!("");
    }

    // Print best configuration
    log::info!("============================================");
    log::info!("✅ Phase 1 Complete!");
    log::info!("");

    if let Some(config) = best_config {
        log::info!("🏆 Best Configuration:");
        log::info!("   CNN weight: {:.2}", config.weight_cnn);
        log::info!("   Rollout weight: {:.2}", config.weight_rollout);
        log::info!("   Heuristic weight: {:.2}", config.weight_heuristic);
        log::info!("   Contextual weight: {:.2}", config.weight_contextual);
        log::info!("   Average Score: {:.2} pts", best_score);
    }

    log::info!("");
    log::info!("Results saved to: {}", args.log_path);
    log::info!("");
    log::info!("📊 To analyze results, run:");
    log::info!(
        "   python3 scripts/analyze_hyperparameters.py {}",
        args.log_path
    );

    Ok(())
}
