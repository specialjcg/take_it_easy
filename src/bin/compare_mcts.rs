use clap::Parser;
use flexi_logger::Logger;
use rand::prelude::IndexedRandom;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::error::Error;

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::mcts::algorithm::{
    mcts_find_best_position_for_tile_pure, mcts_find_best_position_for_tile_with_nn,
};
use take_it_easy::neural::policy_value_net::{PolicyNet, ValueNet};
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(
    name = "compare-mcts",
    about = "Compare pure MCTS against neural-guided MCTS to quantify NN benefit."
)]
struct Args {
    /// Number of self-play games to simulate for each approach
    #[arg(short, long, default_value_t = 100)]
    games: usize,

    /// Number of MCTS simulations per move
    #[arg(short, long, default_value_t = 150)]
    simulations: usize,

    /// RNG seed used to generate identical tile sequences for both agents
    #[arg(short, long, default_value_t = 2025)]
    seed: u64,

    /// Number of turns to play (default 19 for full Take It Easy game)
    #[arg(long, default_value_t = 19)]
    turns: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    log::info!(
        "🎮 Starting comparison: games={}, simulations={}, seed={}, turns={}",
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
        ..Default::default()
    };
    let manager = NeuralManager::with_config(neural_config)?;

    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut pure_scores = Vec::with_capacity(args.games);
    let mut neural_scores = Vec::with_capacity(args.games);
    let mut nn_better = 0usize;
    let mut same_score = 0usize;

    for game_index in 0..args.games {
        let tile_order = sample_tile_sequence(&mut rng, args.turns);
        if tile_order.len() < args.turns {
            log::warn!(
                "Game {} terminated early: only {} tiles generated.",
                game_index,
                tile_order.len()
            );
        }

        let pure_score = play_game(&tile_order, args.simulations, Strategy::Pure);

        let policy_net = manager.policy_net();
        let value_net = manager.value_net();
        let neural_score = play_game(
            &tile_order,
            args.simulations,
            Strategy::Neural {
                policy_net,
                value_net,
            },
        );

        if neural_score > pure_score {
            nn_better += 1;
        } else if neural_score == pure_score {
            same_score += 1;
        }

        pure_scores.push(pure_score);
        neural_scores.push(neural_score);
    }

    let pure_stats = compute_stats(&pure_scores);
    let neural_stats = compute_stats(&neural_scores);
    let delta_mean = neural_stats.mean - pure_stats.mean;

    println!("\n===== MCTS Comparison Summary =====");
    println!("Games simulated    : {}", args.games);
    println!("Simulations/move   : {}", args.simulations);
    println!("Turns per game     : {}", args.turns);
    println!();
    println!(
        "Pure MCTS          : mean = {:>6.2}, std = {:>6.2}, min = {:>4}, max = {:>4}",
        pure_stats.mean, pure_stats.std_dev, pure_stats.min, pure_stats.max
    );
    println!(
        "MCTS + Neural Net  : mean = {:>6.2}, std = {:>6.2}, min = {:>4}, max = {:>4}",
        neural_stats.mean, neural_stats.std_dev, neural_stats.min, neural_stats.max
    );
    println!("Mean score delta   : {:+.2} (NN - Pure)", delta_mean);
    println!(
        "NN better in       : {} / {} games ({:.1}%), equal in {}",
        nn_better,
        args.games,
        (nn_better as f64 / args.games as f64) * 100.0,
        same_score
    );
    println!("===================================\n");

    Ok(())
}

enum Strategy<'a> {
    Pure,
    Neural {
        policy_net: &'a PolicyNet,
        value_net: &'a ValueNet,
    },
}

fn play_game(tiles: &[Tile], num_simulations: usize, strategy: Strategy<'_>) -> i32 {
    let mut deck = create_deck();
    let mut plateau = create_plateau_empty();
    let total_turns = tiles.len();

    for (turn, tile) in tiles.iter().enumerate() {
        let mcts_result = match strategy {
            Strategy::Pure => mcts_find_best_position_for_tile_pure(
                &mut plateau,
                &mut deck,
                *tile,
                num_simulations,
                turn,
                total_turns,
            ),
            Strategy::Neural {
                policy_net,
                value_net,
            } => mcts_find_best_position_for_tile_with_nn(
                &mut plateau,
                &mut deck,
                *tile,
                policy_net,
                value_net,
                num_simulations,
                turn,
                total_turns,
            ),
        };

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
