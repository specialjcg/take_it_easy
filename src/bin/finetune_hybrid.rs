//! Fine-tune Hybrid MCTS parameters
//!
//! Tests different top_k values and turn thresholds to optimize Q-net pruning.

use flexi_logger::Logger;
use rand::prelude::*;
use std::error::Error;

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_pure;
use take_it_easy::neural::policy_value_net::{PolicyNet, ValueNet};
use take_it_easy::neural::qvalue_net::QValueNet;
use take_it_easy::neural::{NeuralConfig, NeuralManager, QNetManager};
use take_it_easy::scoring::scoring::result;

const NUM_GAMES: usize = 50;
const NUM_SIMS: usize = 100;
const SEED: u64 = 2025;

fn main() -> Result<(), Box<dyn Error>> {
    Logger::try_with_env_or_str("warn")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    println!("{}", "=".repeat(70));
    println!("     HYBRID MCTS FINE-TUNING: Testing top_k and turn_threshold");
    println!("{}", "=".repeat(70));
    println!("Games: {}, Simulations: {}", NUM_GAMES, NUM_SIMS);
    println!();

    // Load networks
    let neural_config = NeuralConfig::default();
    let neural_manager = NeuralManager::with_config(neural_config)?;
    let qnet_manager = QNetManager::new("model_weights/qvalue_net.params")?;

    let policy_net = neural_manager.policy_net();
    let value_net = neural_manager.value_net();
    let qvalue_net = qnet_manager.net();

    // Generate shared tile sequences
    let mut rng = rand::rngs::StdRng::seed_from_u64(SEED);
    let all_tiles: Vec<Vec<Tile>> = (0..NUM_GAMES).map(|_| sample_tiles(&mut rng, 19)).collect();

    // Baseline: Pure MCTS
    println!("Running Pure MCTS baseline...");
    let pure_scores: Vec<i32> = all_tiles
        .iter()
        .map(|tiles| play_game_pure(tiles, NUM_SIMS))
        .collect();
    let pure_mean = mean(&pure_scores);
    println!("Pure MCTS baseline: {:.2}\n", pure_mean);

    // Test configurations
    let top_k_values = [4, 6, 8, 10, 12];
    let turn_thresholds = [10, 12, 15, 17];

    println!(
        "{:>8} | {:>15} | {:>8} | {:>8} | {:>8}",
        "top_k", "turn_threshold", "mean", "delta", "wins"
    );
    println!("{}", "-".repeat(60));

    let mut best_config = (0, 0, 0.0);

    for &top_k in &top_k_values {
        for &turn_threshold in &turn_thresholds {
            let scores: Vec<i32> = all_tiles
                .iter()
                .map(|tiles| {
                    play_game_hybrid_configurable(
                        tiles,
                        NUM_SIMS,
                        policy_net,
                        value_net,
                        qvalue_net,
                        top_k,
                        turn_threshold,
                    )
                })
                .collect();

            let hybrid_mean = mean(&scores);
            let delta = hybrid_mean - pure_mean;
            let wins = count_wins(&scores, &pure_scores);

            println!(
                "{:>8} | {:>15} | {:>8.2} | {:>+8.2} | {:>5}/{}",
                top_k, turn_threshold, hybrid_mean, delta, wins, NUM_GAMES
            );

            if hybrid_mean > best_config.2 {
                best_config = (top_k, turn_threshold, hybrid_mean);
            }
        }
    }

    println!("{}", "-".repeat(60));
    println!(
        "\n✅ Best config: top_k={}, turn_threshold={} -> {:.2} pts (+{:.2} vs pure)",
        best_config.0,
        best_config.1,
        best_config.2,
        best_config.2 - pure_mean
    );

    Ok(())
}

fn play_game_pure(tiles: &[Tile], num_sims: usize) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let result = mcts_find_best_position_for_tile_pure(
            &mut plateau,
            &mut deck,
            *tile,
            num_sims,
            turn,
            19,
            None,
        );
        plateau.tiles[result.best_position] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn play_game_hybrid_configurable(
    tiles: &[Tile],
    num_sims: usize,
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    qvalue_net: &QValueNet,
    top_k: usize,
    turn_threshold: usize,
) -> i32 {
    use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;

    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let empty_count = plateau
            .tiles
            .iter()
            .filter(|t| **t == Tile(0, 0, 0))
            .count();

        // Adaptive pruning decision
        let should_prune = empty_count > top_k + 2 && turn < turn_threshold;

        if should_prune {
            // Q-net pruning: get top-K positions and run rollouts on them
            let top_positions = qvalue_net.get_top_positions(&plateau.tiles, tile, top_k);

            let mut best_pos = top_positions[0];
            let mut best_score = i32::MIN;

            for &pos in &top_positions {
                let mut sim_plateau = plateau.clone();
                let mut sim_deck = deck.clone();
                sim_plateau.tiles[pos] = *tile;
                sim_deck = replace_tile_in_deck(&sim_deck, tile);

                let mut total = 0i64;
                for _ in 0..num_sims {
                    total += rollout_score(&sim_plateau, &sim_deck) as i64;
                }
                let avg = (total / num_sims as i64) as i32;

                if avg > best_score {
                    best_score = avg;
                    best_pos = pos;
                }
            }

            plateau.tiles[best_pos] = *tile;
        } else {
            // Late game: use full CNN MCTS
            let result = mcts_find_best_position_for_tile_with_nn(
                &mut plateau,
                &mut deck,
                *tile,
                policy_net,
                value_net,
                num_sims,
                turn,
                19,
                None,
            );
            plateau.tiles[result.best_position] = *tile;
        }

        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn rollout_score(
    plateau: &take_it_easy::game::plateau::Plateau,
    deck: &take_it_easy::game::deck::Deck,
) -> i32 {
    let mut sim_plateau = plateau.clone();
    let mut sim_deck = deck.clone();
    let mut rng = rand::rng();

    loop {
        let available = get_available_tiles(&sim_deck);
        if available.is_empty() {
            break;
        }

        let tile = *available.choose(&mut rng).unwrap();

        let empty_positions: Vec<usize> = sim_plateau
            .tiles
            .iter()
            .enumerate()
            .filter(|(_, t)| **t == Tile(0, 0, 0))
            .map(|(i, _)| i)
            .collect();

        if empty_positions.is_empty() {
            break;
        }

        let pos = *empty_positions.choose(&mut rng).unwrap();
        sim_plateau.tiles[pos] = tile;
        sim_deck = replace_tile_in_deck(&sim_deck, &tile);
    }

    result(&sim_plateau)
}

fn sample_tiles(rng: &mut rand::rngs::StdRng, count: usize) -> Vec<Tile> {
    let mut deck = create_deck();
    let mut tiles = Vec::new();

    for _ in 0..count {
        let available = get_available_tiles(&deck);
        if available.is_empty() {
            break;
        }
        let tile = *available.choose(rng).unwrap();
        tiles.push(tile);
        deck = replace_tile_in_deck(&deck, &tile);
    }

    tiles
}

fn mean(values: &[i32]) -> f64 {
    values.iter().sum::<i32>() as f64 / values.len() as f64
}

fn count_wins(a: &[i32], b: &[i32]) -> usize {
    a.iter().zip(b.iter()).filter(|(x, y)| x > y).count()
}
