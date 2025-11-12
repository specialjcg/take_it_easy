//! Expert Data Generator for Curriculum Learning
//!
//! Generates high-quality training data using either:
//! 1. High-simulation MCTS (quick, decent quality)
//! 2. Beam search (slow, highest quality)

use clap::Parser;
use flexi_logger::Logger;
use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::BufWriter;

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(
    name = "expert-data-generator",
    about = "Generate expert training data for curriculum learning"
)]
struct Args {
    /// Number of games to generate
    #[arg(short, long)]
    num_games: usize,

    /// Number of MCTS simulations per move (expert strength)
    #[arg(short, long, default_value_t = 500)]
    simulations: usize,

    /// Output JSON file
    #[arg(short, long)]
    output: String,

    /// RNG seed for reproducibility
    #[arg(long, default_value_t = 2025)]
    seed: u64,

    /// Generate simplified data (position only, no value distribution)
    #[arg(long, default_value_t = false)]
    simple: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ExpertGame {
    game_id: usize,
    moves: Vec<ExpertMove>,
    final_score: i32,
    seed: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ExpertMove {
    turn: usize,
    plateau_before: Vec<i32>, // 19 cells state before move
    tile: TileData,
    best_position: usize,
    expected_value: f64, // From MCTS value estimate
    #[serde(skip_serializing_if = "Option::is_none")]
    policy_distribution: Option<HashMap<usize, f64>>, // Position -> probability
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct TileData {
    value1: i32,
    value2: i32,
    value3: i32,
}

impl From<&Tile> for TileData {
    fn from(tile: &Tile) -> Self {
        Self {
            value1: tile.0,
            value2: tile.1,
            value3: tile.2,
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    let args = Args::parse();

    log::info!("🎯 Expert Data Generator");
    log::info!("Games: {}", args.num_games);
    log::info!("Simulations per move: {}", args.simulations);
    log::info!("Output: {}", args.output);
    log::info!("Seed: {}", args.seed);

    // Initialize neural network for MCTS guidance
    log::info!("Loading neural network...");
    let neural_config = NeuralConfig {
        input_dim: (8, 5, 5),
        nn_architecture: NNArchitecture::CNN,
        ..Default::default()
    };
    let manager = NeuralManager::with_config(neural_config)?;
    let policy_net = manager.policy_net();
    let value_net = manager.value_net();
    log::info!("✅ Neural network loaded");

    // Generate expert games
    log::info!("\n🚀 Generating {} expert games...", args.num_games);
    let mut expert_games = Vec::new();
    let mut total_score = 0i64;
    let start_time = std::time::Instant::now();

    for game_id in 0..args.num_games {
        let game_seed = args.seed + game_id as u64;
        let mut rng = StdRng::seed_from_u64(game_seed);

        let game = generate_expert_game(
            game_id,
            game_seed,
            &mut rng,
            policy_net,
            value_net,
            args.simulations,
            args.simple,
        );

        total_score += game.final_score as i64;
        expert_games.push(game.clone());

        if (game_id + 1) % 10 == 0 || game_id + 1 == args.num_games {
            let avg_score = total_score as f64 / (game_id + 1) as f64;
            let elapsed = start_time.elapsed();
            let rate = (game_id + 1) as f64 / elapsed.as_secs_f64();
            let eta_secs = (args.num_games - game_id - 1) as f64 / rate;

            log::info!(
                "Progress: {}/{} games | Avg score: {:.1} | Last: {} | Rate: {:.2} games/sec | ETA: {}m {}s",
                game_id + 1,
                args.num_games,
                avg_score,
                game.final_score,
                rate,
                eta_secs as u64 / 60,
                eta_secs as u64 % 60
            );
        }
    }

    let total_time = start_time.elapsed();
    let avg_score = total_score as f64 / args.num_games as f64;

    log::info!("\n📊 Generation Complete!");
    log::info!("Total games: {}", args.num_games);
    log::info!("Average score: {:.2} pts", avg_score);
    log::info!(
        "Total time: {}m {}s",
        total_time.as_secs() / 60,
        total_time.as_secs() % 60
    );
    log::info!("Total training examples: {}", args.num_games * 19);

    // Save to JSON
    log::info!("\n💾 Saving to {}...", args.output);
    let file = File::create(&args.output)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &expert_games)?;
    log::info!("✅ Data saved successfully");

    Ok(())
}

fn generate_expert_game(
    game_id: usize,
    seed: u64,
    rng: &mut StdRng,
    policy_net: &take_it_easy::neural::policy_value_net::PolicyNet,
    value_net: &take_it_easy::neural::policy_value_net::ValueNet,
    simulations: usize,
    simple: bool,
) -> ExpertGame {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();
    let mut moves = Vec::new();

    // Shuffle deck
    let available = get_available_tiles(&deck);
    let mut tile_sequence: Vec<Tile> = available.iter().copied().collect();
    tile_sequence.shuffle(rng);

    // Play 19 turns
    for turn in 0..19 {
        let tile = tile_sequence[turn];

        // Store state before move
        // Encode each tile as: value1*100 + value2*10 + value3, or -1 for empty
        let plateau_before: Vec<i32> = plateau
            .tiles
            .iter()
            .map(|tile| {
                if tile.0 == 0 && tile.1 == 0 && tile.2 == 0 {
                    -1 // Empty cell
                } else {
                    tile.0 * 100 + tile.1 * 10 + tile.2
                }
            })
            .collect();

        // Use MCTS to find best position
        let mcts_result = mcts_find_best_position_for_tile_with_nn(
            &mut plateau,
            &mut deck,
            tile,
            policy_net,
            value_net,
            simulations,
            turn,
            19,
            None,
        );

        let best_position = mcts_result.best_position;
        let expected_value = mcts_result.subscore; // Value estimate from MCTS

        // Get policy distribution if not simple mode
        let policy_distribution = if !simple {
            Some(extract_policy_distribution(&mcts_result))
        } else {
            None
        };

        // Record the expert move
        moves.push(ExpertMove {
            turn,
            plateau_before,
            tile: TileData::from(&tile),
            best_position,
            expected_value,
            policy_distribution,
        });

        // Apply the move
        plateau.tiles[best_position] = tile;
        deck = replace_tile_in_deck(&deck, &tile);
    }

    let final_score = result(&plateau);

    ExpertGame {
        game_id,
        moves,
        final_score,
        seed,
    }
}

fn extract_policy_distribution(
    mcts_result: &take_it_easy::mcts::mcts_result::MCTSResult,
) -> HashMap<usize, f64> {
    // Extract policy distribution from MCTS result
    // The policy_distribution_boosted tensor contains probabilities for each position
    let policy_tensor = &mcts_result.policy_distribution_boosted;

    // Convert tensor to HashMap
    let mut distribution = HashMap::new();

    // Get legal positions (non-zero probabilities)
    let probs: Vec<f64> = policy_tensor
        .to_kind(tch::Kind::Float)
        .view([-1])
        .try_into()
        .unwrap_or_else(|_| vec![]);

    for (pos, &prob) in probs.iter().enumerate() {
        if prob > 1e-6 {
            // Only store non-negligible probabilities
            distribution.insert(pos, prob);
        }
    }

    distribution
}
