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
    name = "grid-search-quick-wins",
    about = "Grid search for Quick Wins: Adaptive Simulations + Temperature Annealing"
)]
struct Args {
    /// Number of games per configuration
    #[arg(short, long, default_value_t = 20)]
    games: usize,

    /// Base number of MCTS simulations per move
    #[arg(short, long, default_value_t = 150)]
    simulations: usize,

    /// RNG seed
    #[arg(short = 'r', long, default_value_t = 2025)]
    seed: u64,

    /// Number of turns to play (default 19 for full game)
    #[arg(long, default_value_t = 19)]
    turns: usize,

    /// CSV path to save results
    #[arg(long, default_value = "quick_wins_grid_search.csv")]
    log_path: String,

    /// Neural network architecture (cnn or gnn)
    #[arg(long, value_enum, default_value = "cnn")]
    nn_architecture: NnArchitectureCli,
}

/// Configuration for Quick Wins grid search
#[derive(Debug, Clone)]
struct QuickWinsConfig {
    // Adaptive simulation multipliers
    sim_mult_early: f64,
    sim_mult_mid: f64,
    sim_mult_late: f64,

    // Temperature annealing
    temp_initial: f64,
    temp_final: f64,
    temp_decay_start: usize,
    temp_decay_end: usize,
}

impl QuickWinsConfig {
    fn to_hyperparameters(&self) -> MCTSHyperparameters {
        MCTSHyperparameters {
            // c_puct (unchanged from Phase 1)
            c_puct_early: 4.2,
            c_puct_mid: 3.8,
            c_puct_late: 3.0,
            variance_mult_high: 1.3,
            variance_mult_low: 0.85,

            // Pruning (unchanged from Phase 1)
            prune_early: 0.05,
            prune_mid1: 0.10,
            prune_mid2: 0.15,
            prune_late: 0.20,

            // Rollouts (unchanged from Phase 1)
            rollout_strong: 3,
            rollout_medium: 5,
            rollout_default: 7,
            rollout_weak: 9,

            // Weights (optimized from Phase 1)
            weight_cnn: 0.65,
            weight_rollout: 0.25,
            weight_heuristic: 0.05,
            weight_contextual: 0.05,

            // Quick Wins parameters (being optimized)
            sim_mult_early: self.sim_mult_early,
            sim_mult_mid: self.sim_mult_mid,
            sim_mult_late: self.sim_mult_late,
            temp_initial: self.temp_initial,
            temp_final: self.temp_final,
            temp_decay_start: self.temp_decay_start,
            temp_decay_end: self.temp_decay_end,
            rave_k: 10.0,
        }
    }

    fn config_name(&self) -> String {
        format!(
            "sim[{:.2},{:.2},{:.2}]_temp[{:.2},{:.2},{}..{}]",
            self.sim_mult_early,
            self.sim_mult_mid,
            self.sim_mult_late,
            self.temp_initial,
            self.temp_final,
            self.temp_decay_start,
            self.temp_decay_end
        )
    }
}

/// Generate grid of Quick Wins configurations to test
fn generate_quick_wins_grid() -> Vec<QuickWinsConfig> {
    let mut configs = Vec::new();

    // Test different simulation budget distributions
    let sim_early_values = [0.50, 0.67, 0.80]; // 75, 100, 120 sims (from base 150)
    let sim_mid_values = [0.90, 1.0, 1.10]; // 135, 150, 165 sims
    let sim_late_values = [1.33, 1.67, 2.0]; // 200, 250, 300 sims

    // Test different temperature schedules
    let temp_initial_values = [1.2, 1.5, 1.8]; // Initial exploration
    let temp_final_values = [0.3, 0.5, 0.7]; // Final exploitation
    let decay_start_values = [3, 5, 7]; // When to start annealing
    let decay_end_values = [13, 15, 17]; // When to finish annealing

    // First: Test simulation schedules (keeping temp at baseline)
    for &sim_early in &sim_early_values {
        for &sim_mid in &sim_mid_values {
            for &sim_late in &sim_late_values {
                configs.push(QuickWinsConfig {
                    sim_mult_early: sim_early,
                    sim_mult_mid: sim_mid,
                    sim_mult_late: sim_late,
                    // Baseline temperature
                    temp_initial: 1.5,
                    temp_final: 0.5,
                    temp_decay_start: 5,
                    temp_decay_end: 15,
                });
            }
        }
    }

    // Second: Test temperature schedules (keeping sims at best from above or baseline)
    for &temp_init in &temp_initial_values {
        for &temp_fin in &temp_final_values {
            for &decay_start in &decay_start_values {
                for &decay_end in &decay_end_values {
                    if decay_end > decay_start + 3 {
                        // Ensure reasonable decay window
                        configs.push(QuickWinsConfig {
                            // Baseline simulations (will be updated after first round)
                            sim_mult_early: 0.67,
                            sim_mult_mid: 1.0,
                            sim_mult_late: 1.67,
                            temp_initial: temp_init,
                            temp_final: temp_fin,
                            temp_decay_start: decay_start,
                            temp_decay_end: decay_end,
                        });
                    }
                }
            }
        }
    }

    configs
}

/// Play one game with given hyperparameters
fn play_game(
    hyperparams: &MCTSHyperparameters,
    num_simulations: usize,
    total_turns: usize,
    nn_manager: &NeuralManager,
    rng: &mut StdRng,
) -> f64 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for turn_idx in 0..total_turns {
        let available_tiles = get_available_tiles(&deck);
        let chosen_tile = *available_tiles.choose(rng).expect("No tiles available");

        let mcts_result = mcts_find_best_position_for_tile_with_nn(
            &mut plateau,
            &mut deck,
            chosen_tile,
            nn_manager.policy_net(),
            nn_manager.value_net(),
            num_simulations,
            turn_idx,
            total_turns,
            Some(hyperparams),
        );

        let best_position = mcts_result.best_position;
        plateau.tiles[best_position] = chosen_tile;
        deck = replace_tile_in_deck(&deck, &chosen_tile);
    }

    result(&plateau) as f64
}

/// Test one configuration over multiple games
fn test_configuration(
    config: &QuickWinsConfig,
    args: &Args,
    nn_manager: &NeuralManager,
    base_rng: &mut StdRng,
) -> (f64, f64, f64, f64) {
    let hyperparams = config.to_hyperparameters();
    let mut scores = Vec::with_capacity(args.games);

    for _ in 0..args.games {
        let score = play_game(
            &hyperparams,
            args.simulations,
            args.turns,
            nn_manager,
            base_rng,
        );
        scores.push(score);
    }

    let avg: f64 = scores.iter().sum::<f64>() / scores.len() as f64;
    let variance: f64 = scores.iter().map(|s| (s - avg).powi(2)).sum::<f64>() / scores.len() as f64;
    let std_dev = variance.sqrt();
    let min = scores.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    (avg, std_dev, min, max)
}

/// Write header to CSV file
fn write_csv_header(log_path: &str) -> Result<(), Box<dyn Error>> {
    if !Path::new(log_path).exists() {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(log_path)?;

        writeln!(
            file,
            "timestamp,config_name,sim_early,sim_mid,sim_late,temp_initial,temp_final,temp_start,temp_end,avg_score,std_dev,min_score,max_score"
        )?;
    }
    Ok(())
}

/// Append result to CSV file
fn append_result(
    log_path: &str,
    config: &QuickWinsConfig,
    avg: f64,
    std_dev: f64,
    min: f64,
    max: f64,
) -> Result<(), Box<dyn Error>> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;

    writeln!(
        file,
        "{},{},{:.2},{:.2},{:.2},{:.2},{:.2},{},{},{:.2},{:.2},{:.0},{:.0}",
        Utc::now().format("%Y-%m-%d %H:%M:%S"),
        config.config_name(),
        config.sim_mult_early,
        config.sim_mult_mid,
        config.sim_mult_late,
        config.temp_initial,
        config.temp_final,
        config.temp_decay_start,
        config.temp_decay_end,
        avg,
        std_dev,
        min,
        max
    )?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    Logger::try_with_env_or_str("info")?.start()?;

    let args = Args::parse();
    let mut rng = StdRng::seed_from_u64(args.seed);

    log::info!("🔍 Quick Wins Grid Search: Adaptive Simulations + Temperature Annealing");
    log::info!("============================================");
    log::info!("Games per config: {}", args.games);
    log::info!("Base simulations: {}", args.simulations);
    log::info!("Seed: {}", args.seed);
    log::info!("Log file: {}", args.log_path);
    log::info!("");

    // Load neural networks
    let nn_config = NeuralConfig {
        input_dim: (8, 5, 5),
        nn_architecture: args.nn_architecture.clone().into(),
        ..Default::default()
    };
    let nn_manager = NeuralManager::with_config(nn_config)?;
    log::info!("✅ Neural networks loaded: {}", args.nn_architecture);
    log::info!("");

    // Generate grid
    let configs = generate_quick_wins_grid();
    log::info!("Total configurations to test: {}", configs.len());
    log::info!("");

    // Write CSV header
    write_csv_header(&args.log_path)?;

    // Test each configuration
    let mut best_score = 0.0;
    let mut best_config: Option<QuickWinsConfig> = None;

    for (i, config) in configs.iter().enumerate() {
        log::info!(
            "[{}/{}] Testing: {}",
            i + 1,
            configs.len(),
            config.config_name()
        );

        let (avg, std_dev, min, max) = test_configuration(config, &args, &nn_manager, &mut rng);

        log::info!(
            "   Result: {:.2} ± {:.2} pts (range: {:.0}-{:.0})",
            avg,
            std_dev,
            min,
            max
        );
        log::info!("");

        append_result(&args.log_path, config, avg, std_dev, min, max)?;

        if avg > best_score {
            best_score = avg;
            best_config = Some(config.clone());
        }
    }

    // Report best configuration
    log::info!("============================================");
    log::info!("✅ Quick Wins Grid Search Complete!");
    log::info!("");

    if let Some(best) = best_config {
        log::info!("🏆 Best Configuration:");
        log::info!("   Sim Early mult: {:.2}", best.sim_mult_early);
        log::info!("   Sim Mid mult: {:.2}", best.sim_mult_mid);
        log::info!("   Sim Late mult: {:.2}", best.sim_mult_late);
        log::info!("   Temp Initial: {:.2}", best.temp_initial);
        log::info!("   Temp Final: {:.2}", best.temp_final);
        log::info!(
            "   Temp Decay: turns {}-{}",
            best.temp_decay_start,
            best.temp_decay_end
        );
        log::info!("   Average Score: {:.2} pts", best_score);
        log::info!("");
    }

    log::info!("Results saved to: {}", args.log_path);
    log::info!("");

    Ok(())
}
