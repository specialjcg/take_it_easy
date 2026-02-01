//! Compare MCTS strategies: Pure vs CNN vs CNN+Q-net (Hybrid)
//!
//! This benchmark tests the integration of Q-net pruning with CNN policy/value.

use clap::Parser;
use flexi_logger::Logger;
use rand::prelude::*;
use std::error::Error;

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::mcts::algorithm::{
    mcts_find_best_position_for_tile_pure, mcts_find_best_position_for_tile_with_nn,
    mcts_find_best_position_for_tile_with_qnet,
};
use take_it_easy::neural::{NeuralConfig, NeuralManager, QNetManager};
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(
    name = "compare-mcts-hybrid",
    about = "Compare Pure vs CNN vs CNN+Q-net MCTS"
)]
struct Args {
    #[arg(short, long, default_value_t = 50)]
    games: usize,

    #[arg(short, long, default_value_t = 100)]
    simulations: usize,

    #[arg(short, long, default_value_t = 2025)]
    seed: u64,

    /// Top-K positions for Q-net pruning (fine-tuned optimal: 6)
    #[arg(long, default_value_t = 6)]
    top_k: usize,

    /// Path to Q-value network weights
    #[arg(long, default_value = "model_weights/qvalue_net.params")]
    qnet_path: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    log::info!("🎮 MCTS Hybrid Comparison: Pure vs CNN vs CNN+Q-net");
    log::info!(
        "Games: {}, Simulations: {}, Top-K: {}",
        args.games,
        args.simulations,
        args.top_k
    );

    // Load CNN policy/value networks
    let neural_config = NeuralConfig::default();
    let neural_manager = NeuralManager::with_config(neural_config)?;
    log::info!("✅ Loaded CNN policy/value networks");

    // Load Q-net
    let qnet_manager = QNetManager::new(&args.qnet_path)?;
    log::info!("✅ Loaded Q-net from {}", args.qnet_path);

    let mut rng = rand::rngs::StdRng::seed_from_u64(args.seed);

    let mut pure_scores = Vec::new();
    let mut cnn_scores = Vec::new();
    let mut hybrid_scores = Vec::new();

    for game_idx in 0..args.games {
        let tiles = sample_tiles(&mut rng, 19);

        // Pure MCTS
        let pure_score = play_game_pure(&tiles, args.simulations);

        // CNN MCTS
        let cnn_score = play_game_cnn(&tiles, args.simulations, &neural_manager);

        // Hybrid (CNN + Q-net pruning)
        let hybrid_score = play_game_hybrid(
            &tiles,
            args.simulations,
            &neural_manager,
            &qnet_manager,
            args.top_k,
        );

        pure_scores.push(pure_score);
        cnn_scores.push(cnn_score);
        hybrid_scores.push(hybrid_score);

        if (game_idx + 1) % 10 == 0 {
            log::info!("Progress: {}/{}", game_idx + 1, args.games);
        }
    }

    // Statistics
    let pure_mean = mean(&pure_scores);
    let cnn_mean = mean(&cnn_scores);
    let hybrid_mean = mean(&hybrid_scores);

    let pure_std = std_dev(&pure_scores);
    let cnn_std = std_dev(&cnn_scores);
    let hybrid_std = std_dev(&hybrid_scores);

    let cnn_wins = count_wins(&cnn_scores, &pure_scores);
    let hybrid_wins_vs_pure = count_wins(&hybrid_scores, &pure_scores);
    let hybrid_wins_vs_cnn = count_wins(&hybrid_scores, &cnn_scores);

    println!("\n{}", "=".repeat(65));
    println!("     MCTS HYBRID COMPARISON: Pure vs CNN vs CNN+Q-net");
    println!("{}", "=".repeat(65));
    println!(
        "Games: {}, Simulations: {}, Top-K: {}",
        args.games, args.simulations, args.top_k
    );
    println!();
    println!(
        "{:<15} : mean={:>6.2}, std={:>5.2}",
        "Pure MCTS", pure_mean, pure_std
    );
    println!(
        "{:<15} : mean={:>6.2}, std={:>5.2}, delta={:+.2}, wins={}/{}",
        "CNN MCTS",
        cnn_mean,
        cnn_std,
        cnn_mean - pure_mean,
        cnn_wins,
        args.games
    );
    println!(
        "{:<15} : mean={:>6.2}, std={:>5.2}, delta={:+.2}, wins={}/{} (vs Pure), {}/{} (vs CNN)",
        "Hybrid MCTS",
        hybrid_mean,
        hybrid_std,
        hybrid_mean - pure_mean,
        hybrid_wins_vs_pure,
        args.games,
        hybrid_wins_vs_cnn,
        args.games
    );
    println!("{}\n", "=".repeat(65));

    // Interpretation
    println!("📈 INTERPRETATION:");
    if hybrid_mean > cnn_mean + 1.0 {
        println!(
            "  ✅ HYBRID improves over CNN by {:.2} pts - Q-net pruning helps!",
            hybrid_mean - cnn_mean
        );
    } else if hybrid_mean > pure_mean + 1.0 && hybrid_mean >= cnn_mean - 1.0 {
        println!(
            "  ➖ HYBRID similar to CNN ({:+.2} pts) - Q-net doesn't hurt",
            hybrid_mean - cnn_mean
        );
    } else {
        println!("  ❌ HYBRID underperforms - Q-net pruning may be too aggressive");
    }

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

fn play_game_cnn(tiles: &[Tile], num_sims: usize, manager: &NeuralManager) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    let policy_net = manager.policy_net();
    let value_net = manager.value_net();

    for (turn, tile) in tiles.iter().enumerate() {
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
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn play_game_hybrid(
    tiles: &[Tile],
    num_sims: usize,
    neural_manager: &NeuralManager,
    qnet_manager: &QNetManager,
    top_k: usize,
) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    let policy_net = neural_manager.policy_net();
    let value_net = neural_manager.value_net();
    let qvalue_net = qnet_manager.net();

    for (turn, tile) in tiles.iter().enumerate() {
        let result = mcts_find_best_position_for_tile_with_qnet(
            &mut plateau,
            &mut deck,
            *tile,
            policy_net,
            value_net,
            qvalue_net,
            num_sims,
            turn,
            19,
            top_k,
            None,
        );
        plateau.tiles[result.best_position] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
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

fn std_dev(values: &[i32]) -> f64 {
    let m = mean(values);
    let variance =
        values.iter().map(|&v| (v as f64 - m).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt()
}

fn count_wins(a: &[i32], b: &[i32]) -> usize {
    a.iter().zip(b.iter()).filter(|(x, y)| x > y).count()
}
