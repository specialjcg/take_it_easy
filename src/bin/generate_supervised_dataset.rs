//! Pure MCTS Dataset Generator for GNN Supervised Learning
//!
//! Generates high-quality training data using pure MCTS with rollouts (no neural network).
//! Output format: CSV with (board_state, tile, position, final_score) for each move.

use clap::Parser;
use flexi_logger::Logger;
use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_pure;
use take_it_easy::mcts::hyperparameters::MCTSHyperparameters;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(
    name = "generate-supervised-dataset",
    about = "Generate supervised learning dataset from pure MCTS gameplay"
)]
struct Args {
    /// Number of games to generate
    #[arg(short, long, default_value_t = 10000)]
    num_games: usize,

    /// Number of MCTS simulations per move
    #[arg(short, long, default_value_t = 300)]
    simulations: usize,

    /// Output CSV file
    #[arg(short, long, default_value = "supervised_dataset.csv")]
    output: String,

    /// RNG seed for reproducibility
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Progress reporting interval
    #[arg(long, default_value_t = 100)]
    report_every: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    let args = Args::parse();

    log::info!("🎯 Supervised Dataset Generator (Pure MCTS)");
    log::info!("Target games: {}", args.num_games);
    log::info!("MCTS simulations: {}", args.simulations);
    log::info!("Output: {}", args.output);
    log::info!("Seed: {}", args.seed);

    let mut rng = StdRng::seed_from_u64(args.seed);
    let hyperparams = MCTSHyperparameters::default();

    // Create output file with header
    let file = File::create(&args.output)?;
    let mut writer = BufWriter::new(file);

    // CSV Header: game_id, turn, plateau_state (19 values × 3 orientations = 57 cols),
    // tile (3 values), position, final_score
    writeln!(writer, "game_id,turn,plateau_0,plateau_1,plateau_2,plateau_3,plateau_4,plateau_5,plateau_6,plateau_7,plateau_8,plateau_9,plateau_10,plateau_11,plateau_12,plateau_13,plateau_14,plateau_15,plateau_16,plateau_17,plateau_18,tile_0,tile_1,tile_2,position,final_score")?;

    let mut total_examples = 0;
    let mut score_sum = 0;
    let mut score_min = i32::MAX;
    let mut score_max = i32::MIN;

    for game_id in 0..args.num_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let turns_per_game = 19;

        // Record all moves in this game
        let mut game_moves: Vec<(Plateau, Tile, usize, usize)> = Vec::new();

        // Play full game with pure MCTS
        for turn in 0..turns_per_game {
            let available_tiles = get_available_tiles(&deck);
            if available_tiles.is_empty() {
                break;
            }

            // Random tile selection (as in real game)
            let chosen_tile = available_tiles[rng.gen_range(0..available_tiles.len())];

            // Use pure MCTS to find best position
            let mcts_result = mcts_find_best_position_for_tile_pure(
                &mut plateau,
                &mut deck,
                chosen_tile,
                args.simulations,
                turn,
                turns_per_game,
                Some(&hyperparams),
            );

            // Record this move (we'll write final_score after game completes)
            game_moves.push((
                plateau.clone(),
                chosen_tile,
                mcts_result.best_position,
                turn,
            ));

            // Apply move
            plateau.tiles[mcts_result.best_position] = chosen_tile;
            deck = replace_tile_in_deck(&deck, &chosen_tile);
        }

        // Calculate final score
        let final_score = result(&plateau);
        score_sum += final_score;
        score_min = score_min.min(final_score);
        score_max = score_max.max(final_score);

        // Write all moves from this game to CSV
        for (plateau_before, tile, position, turn) in game_moves {
            write!(writer, "{},{}", game_id, turn)?;

            // Write plateau state (19 positions × 3 orientations, but we'll flatten to just positions)
            for pos in 0..19 {
                let t = plateau_before.tiles[pos];
                // Encode tile as single value: combine 3 orientations
                let encoded = if t == Tile(0, 0, 0) {
                    0
                } else {
                    t.0 * 100 + t.1 * 10 + t.2
                };
                write!(writer, ",{}", encoded)?;
            }

            // Write tile, position, and final score
            writeln!(
                writer,
                ",{},{},{},{},{}",
                tile.0, tile.1, tile.2, position, final_score
            )?;
            total_examples += 1;
        }

        // Progress reporting
        if (game_id + 1) % args.report_every == 0 {
            let avg_score = score_sum as f64 / (game_id + 1) as f64;
            log::info!(
                "Progress: {}/{} games | Avg score: {:.1} | Range: [{}, {}] | Examples: {}",
                game_id + 1,
                args.num_games,
                avg_score,
                score_min,
                score_max,
                total_examples
            );
        }
    }

    writer.flush()?;

    let final_avg = score_sum as f64 / args.num_games as f64;
    log::info!("✅ Dataset generation complete!");
    log::info!("Total examples: {}", total_examples);
    log::info!("Average score: {:.1}", final_avg);
    log::info!("Score range: [{}, {}]", score_min, score_max);
    log::info!("Output: {}", args.output);

    Ok(())
}
