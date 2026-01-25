//! Compare MCTS with Q-Value Network vs Pure MCTS
//!
//! Uses the trained Q-Value network to predict position values directly,
//! replacing or augmenting rollout simulations.

use clap::Parser;
use flexi_logger::Logger;
use rand::prelude::*;
use std::error::Error;
use tch::{nn, Device, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::simulate_game::simulate_games;
use take_it_easy::game::tile::Tile;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "compare-mcts-qvalue", about = "Compare Q-Value Network guided MCTS vs Pure MCTS")]
struct Args {
    #[arg(short, long, default_value_t = 50)]
    games: usize,

    #[arg(short, long, default_value_t = 100)]
    simulations: usize,

    #[arg(short, long, default_value_t = 2025)]
    seed: u64,

    /// Prior strength for Q-network (like virtual visits, higher = more trust in Q-net)
    #[arg(long, default_value_t = 2.0)]
    prior_strength: f64,

    /// Path to Q-value network weights
    #[arg(long, default_value = "model_weights/qvalue_net.params")]
    qnet_path: String,
}

/// Q-Value Network (must match training architecture)
struct QValueNet {
    conv1: nn::Conv2D,
    bn1: nn::BatchNorm,
    conv2: nn::Conv2D,
    bn2: nn::BatchNorm,
    conv3: nn::Conv2D,
    bn3: nn::BatchNorm,
    fc1: nn::Linear,
    fc2: nn::Linear,
    qvalue_head: nn::Linear,
}

impl QValueNet {
    fn new(vs: &nn::VarStore) -> Self {
        let p = vs.root();

        let conv1 = nn::conv2d(&p / "conv1", 47, 64, 3, nn::ConvConfig { padding: 1, ..Default::default() });
        let bn1 = nn::batch_norm2d(&p / "bn1", 64, Default::default());

        let conv2 = nn::conv2d(&p / "conv2", 64, 128, 3, nn::ConvConfig { padding: 1, ..Default::default() });
        let bn2 = nn::batch_norm2d(&p / "bn2", 128, Default::default());

        let conv3 = nn::conv2d(&p / "conv3", 128, 128, 3, nn::ConvConfig { padding: 1, ..Default::default() });
        let bn3 = nn::batch_norm2d(&p / "bn3", 128, Default::default());

        let fc1 = nn::linear(&p / "fc1", 128 * 5 * 5, 512, Default::default());
        let fc2 = nn::linear(&p / "fc2", 512, 256, Default::default());
        let qvalue_head = nn::linear(&p / "qvalue_head", 256, 19, Default::default());

        Self { conv1, bn1, conv2, bn2, conv3, bn3, fc1, fc2, qvalue_head }
    }

    fn forward(&self, x: &Tensor) -> Tensor {
        let h = x.apply(&self.conv1).apply_t(&self.bn1, false).relu();
        let h = h.apply(&self.conv2).apply_t(&self.bn2, false).relu();
        let h = h.apply(&self.conv3).apply_t(&self.bn3, false).relu();
        let h = h.flat_view();
        let h = h.apply(&self.fc1).relu();
        let h = h.apply(&self.fc2).relu();
        h.apply(&self.qvalue_head)
    }

    /// Predict Q-values for all 19 positions
    fn predict_qvalues(&self, plateau: &[Tile], tile: &Tile) -> [f64; 19] {
        let state = encode_state(plateau, tile);
        let input = Tensor::from_slice(&state)
            .view([1, 47, 5, 5])
            .to_kind(Kind::Float);

        let output = self.forward(&input);
        let mut qvalues = [0.0f64; 19];

        for i in 0..19 {
            qvalues[i] = output.double_value(&[0, i as i64]);
        }

        qvalues
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    log::info!("🎮 Q-Value Network MCTS Comparison (Progressive Prior Mode)");
    log::info!("Games: {}, Simulations: {}, Prior Strength: {}", args.games, args.simulations, args.prior_strength);

    // Load Q-Value network
    let mut vs = nn::VarStore::new(Device::Cpu);
    let qnet = QValueNet::new(&vs);
    vs.load(&args.qnet_path)?;
    log::info!("✅ Loaded Q-Value network from {}", args.qnet_path);

    // Debug: test network predictions on empty board
    {
        let empty_plateau = create_plateau_empty();
        let test_tile = Tile(1, 2, 3);
        let qvalues = qnet.predict_qvalues(&empty_plateau.tiles, &test_tile);
        log::info!("🔍 Debug: Q-values for empty board with tile (1,2,3):");
        log::info!("   Raw Q-values: {:?}", qvalues);
        log::info!("   Scaled (x200): {:?}", qvalues.iter().map(|v| v * 200.0).collect::<Vec<_>>());
    }

    let mut rng = rand::rngs::StdRng::seed_from_u64(args.seed);
    let mut pure_scores = Vec::new();
    let mut qnet_scores = Vec::new();

    for game_idx in 0..args.games {
        let tiles = sample_tiles(&mut rng, 19);

        // Pure MCTS
        let pure_score = play_game_pure(&tiles, args.simulations);

        // Q-Network guided
        let qnet_score = play_game_qnet(&tiles, args.simulations, &qnet, args.prior_strength);

        pure_scores.push(pure_score);
        qnet_scores.push(qnet_score);

        if (game_idx + 1) % 10 == 0 {
            log::info!("Progress: {}/{}", game_idx + 1, args.games);
        }
    }

    // Statistics
    let pure_mean = pure_scores.iter().sum::<i32>() as f64 / args.games as f64;
    let qnet_mean = qnet_scores.iter().sum::<i32>() as f64 / args.games as f64;
    let pure_std = std_dev(&pure_scores);
    let qnet_std = std_dev(&qnet_scores);
    let delta = qnet_mean - pure_mean;

    let qnet_wins = pure_scores.iter().zip(&qnet_scores)
        .filter(|(p, q)| q > p).count();

    println!("\n===== Q-Value MCTS (Progressive Prior) =====");
    println!("Games: {}, Simulations: {}", args.games, args.simulations);
    println!("Prior Strength: {} (virtual visits)", args.prior_strength);
    println!();
    println!("Pure MCTS      : mean = {:>6.2}, std = {:>6.2}", pure_mean, pure_std);
    println!("Q-Net Prior    : mean = {:>6.2}, std = {:>6.2}", qnet_mean, qnet_std);
    println!("Delta          : {:+.2}", delta);
    println!("Q-Net wins     : {} / {} ({:.1}%)", qnet_wins, args.games, qnet_wins as f64 / args.games as f64 * 100.0);
    println!("============================================\n");

    Ok(())
}

fn play_game_pure(tiles: &[Tile], num_sims: usize) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for tile in tiles {
        let best_pos = find_best_position_pure(&plateau, &deck, *tile, num_sims);
        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn play_game_qnet(tiles: &[Tile], num_sims: usize, qnet: &QValueNet, prior_strength: f64) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for tile in tiles {
        let best_pos = find_best_position_qnet(&plateau, &deck, *tile, num_sims, qnet, prior_strength);
        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn find_best_position_pure(plateau: &take_it_easy::game::plateau::Plateau, deck: &take_it_easy::game::deck::Deck, tile: Tile, num_sims: usize) -> usize {
    let empty_positions: Vec<usize> = (0..19)
        .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
        .collect();

    let mut best_pos = empty_positions[0];
    let mut best_value = f64::MIN;

    for &pos in &empty_positions {
        let mut temp_plateau = plateau.clone();
        temp_plateau.tiles[pos] = tile;
        let temp_deck = replace_tile_in_deck(deck, &tile);

        let mut total = 0.0;
        for _ in 0..num_sims {
            total += simulate_games(temp_plateau.clone(), temp_deck.clone()) as f64;
        }
        let avg = total / num_sims as f64;

        if avg > best_value {
            best_value = avg;
            best_pos = pos;
        }
    }

    best_pos
}

fn find_best_position_qnet(
    plateau: &take_it_easy::game::plateau::Plateau,
    deck: &take_it_easy::game::deck::Deck,
    tile: Tile,
    num_sims: usize,
    qnet: &QValueNet,
    prior_strength: f64,  // Weight of Q-net prior in final decision
) -> usize {
    let empty_positions: Vec<usize> = (0..19)
        .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
        .collect();

    // Get Q-network predictions
    let qvalues = qnet.predict_qvalues(&plateau.tiles, &tile);

    let mut best_pos = empty_positions[0];
    let mut best_value = f64::MIN;

    for &pos in &empty_positions {
        // Run rollouts for this position (same as pure MCTS)
        let mut temp_plateau = plateau.clone();
        temp_plateau.tiles[pos] = tile;
        let temp_deck = replace_tile_in_deck(deck, &tile);

        let mut total = 0.0;
        for _ in 0..num_sims {
            total += simulate_games(temp_plateau.clone(), temp_deck.clone()) as f64;
        }
        let rollout_avg = total / num_sims as f64;

        // Q-net value as prior (scaled)
        let qnet_value = qvalues[pos] * 200.0;

        // Combine using virtual counts formula:
        // value = (num_sims * rollout_avg + prior_strength * qnet_value) / (num_sims + prior_strength)
        // This is equivalent to treating Q-net as "prior_strength" virtual samples
        let combined = (num_sims as f64 * rollout_avg + prior_strength * qnet_value)
            / (num_sims as f64 + prior_strength);

        if combined > best_value {
            best_value = combined;
            best_pos = pos;
        }
    }

    best_pos
}

fn sample_tiles(rng: &mut rand::rngs::StdRng, count: usize) -> Vec<Tile> {
    let mut deck = create_deck();
    let mut tiles = Vec::new();

    for _ in 0..count {
        let available = get_available_tiles(&deck);
        if available.is_empty() { break; }
        let tile = *available.choose(rng).unwrap();
        tiles.push(tile);
        deck = replace_tile_in_deck(&deck, &tile);
    }

    tiles
}

fn std_dev(values: &[i32]) -> f64 {
    let mean = values.iter().sum::<i32>() as f64 / values.len() as f64;
    let variance = values.iter()
        .map(|&v| (v as f64 - mean).powi(2))
        .sum::<f64>() / values.len() as f64;
    variance.sqrt()
}

const HEX_TO_GRID: [(usize, usize); 19] = [
    (1, 0), (2, 0), (3, 0),
    (0, 1), (1, 1), (2, 1), (3, 1),
    (0, 2), (1, 2), (2, 2), (3, 2), (4, 2),
    (0, 3), (1, 3), (2, 3), (3, 3),
    (1, 4), (2, 4), (3, 4),
];

fn hex_to_grid_idx(hex_pos: usize) -> usize {
    let (row, col) = HEX_TO_GRID[hex_pos];
    row * 5 + col
}

fn encode_state(plateau: &[Tile], tile: &Tile) -> Vec<f32> {
    let mut state = vec![0.0f32; 47 * 5 * 5];

    let num_placed = plateau.iter().filter(|t| **t != Tile(0, 0, 0)).count();
    let turn_progress = num_placed as f32 / 19.0;

    for (hex_pos, t) in plateau.iter().enumerate() {
        let grid_idx = hex_to_grid_idx(hex_pos);

        if *t == Tile(0, 0, 0) {
            state[3 * 25 + grid_idx] = 1.0;
        } else {
            state[grid_idx] = t.0 as f32 / 9.0;
            state[25 + grid_idx] = t.1 as f32 / 9.0;
            state[2 * 25 + grid_idx] = t.2 as f32 / 9.0;
        }

        state[4 * 25 + grid_idx] = tile.0 as f32 / 9.0;
        state[5 * 25 + grid_idx] = tile.1 as f32 / 9.0;
        state[6 * 25 + grid_idx] = tile.2 as f32 / 9.0;
        state[7 * 25 + grid_idx] = turn_progress;
    }

    state
}
