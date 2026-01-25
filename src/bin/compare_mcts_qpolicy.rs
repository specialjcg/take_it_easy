//! Compare MCTS strategies using Q-Value Network as Prior Policy
//!
//! Instead of adding Q-values as bonus to UCB score, these strategies use
//! Q-net to GUIDE exploration like AlphaZero uses policy network:
//!
//! 1. ACTION PRUNING: Only explore top-K positions according to Q-net
//! 2. PROGRESSIVE BIAS: Q-net influence decreases as visit count increases
//! 3. LEAF INIT: Use Q(s,a) as initial value before any rollout
//!
//! This is the key insight: networks beat rollouts when used as PRIOR, not bonus.

use clap::Parser;
use flexi_logger::Logger;
use rand::prelude::*;
use std::collections::HashMap;
use std::error::Error;
use tch::{nn, Device, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::deck::Deck;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::simulate_game_smart::simulate_games_smart;
use take_it_easy::game::tile::Tile;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "compare-mcts-qpolicy", about = "Compare Q-Net as Prior Policy strategies")]
struct Args {
    #[arg(short, long, default_value_t = 50)]
    games: usize,

    #[arg(short, long, default_value_t = 100)]
    simulations: usize,

    #[arg(short, long, default_value_t = 2025)]
    seed: u64,

    /// Strategy: pure, prune, progressive, leaf, combined
    #[arg(long, default_value = "all")]
    strategy: String,

    /// Top-K positions to explore (for prune strategy)
    #[arg(long, default_value_t = 5)]
    top_k: usize,

    /// Progressive bias coefficient (higher = more Q-net influence early)
    #[arg(long, default_value_t = 2.0)]
    bias_coef: f64,

    /// Path to Q-value network weights
    #[arg(long, default_value = "model_weights/qvalue_net.params")]
    qnet_path: String,
}

/// Q-Value Network (same architecture as training)
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

    /// Predict Q-values for all 19 positions (with softmax for ranking)
    fn predict_qvalues(&self, plateau: &[Tile], tile: &Tile) -> [f64; 19] {
        let state = encode_state(plateau, tile);
        let input = Tensor::from_slice(&state)
            .view([1, 47, 5, 5])
            .to_kind(Kind::Float);

        let output = self.forward(&input);

        // Apply softmax since model was trained on softmax targets
        let output_softmax = output.softmax(-1, Kind::Float);

        let mut qvalues = [0.0f64; 19];
        for i in 0..19 {
            qvalues[i] = output_softmax.double_value(&[0, i as i64]);
        }

        qvalues
    }

    /// Predict Q-values and convert to softmax policy (for ranking)
    /// Temperature controls sharpness: lower = more greedy
    fn predict_policy(&self, plateau: &[Tile], tile: &Tile, temperature: f64) -> [f64; 19] {
        let qvalues = self.predict_qvalues(plateau, tile);

        // Only consider valid (empty) positions
        let mut valid_positions = Vec::new();
        for (pos, t) in plateau.iter().enumerate() {
            if *t == Tile(0, 0, 0) {
                valid_positions.push(pos);
            }
        }

        if valid_positions.is_empty() {
            return qvalues;
        }

        // Softmax over valid positions only
        let max_q = valid_positions.iter()
            .map(|&pos| qvalues[pos])
            .fold(f64::NEG_INFINITY, f64::max);

        let mut exp_sum = 0.0;
        let mut exp_values = [0.0f64; 19];

        for &pos in &valid_positions {
            let exp_val = ((qvalues[pos] - max_q) / temperature).exp();
            exp_values[pos] = exp_val;
            exp_sum += exp_val;
        }

        // Normalize
        let mut policy = [0.0f64; 19];
        if exp_sum > 0.0 {
            for &pos in &valid_positions {
                policy[pos] = exp_values[pos] / exp_sum;
            }
        }

        policy
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    log::info!("🎯 Q-Value Network as Prior Policy Comparison");
    log::info!("Games: {}, Simulations: {}", args.games, args.simulations);

    // Load Q-Value network
    let mut vs = nn::VarStore::new(Device::Cpu);
    let qnet = QValueNet::new(&vs);
    vs.load(&args.qnet_path)?;
    log::info!("✅ Loaded Q-Value network from {}", args.qnet_path);

    // Debug: RANKING ACCURACY TEST
    // If Q-net ranking is useful, the best Q-net position should often be the best rollout position
    {
        log::info!("🔍 RANKING ACCURACY TEST (10 random states):");
        let mut correct_top1 = 0;
        let mut correct_top3 = 0;
        let mut total_tests = 0;

        let mut test_rng = rand::rngs::StdRng::seed_from_u64(12345);
        let test_tiles = sample_tiles(&mut test_rng, 5);

        for (i, &test_tile) in test_tiles.iter().enumerate() {
            let mut plateau = create_plateau_empty();
            // Place i tiles randomly to create different game states
            for j in 0..i {
                let pos = (j * 4) % 19;
                plateau.tiles[pos] = Tile(1, 2, 3);
            }

            let deck = create_deck();
            let qvalues = qnet.predict_qvalues(&plateau.tiles, &test_tile);

            // Get all empty positions
            let empty: Vec<usize> = (0..19)
                .filter(|&p| plateau.tiles[p] == Tile(0, 0, 0))
                .collect();

            if empty.len() < 3 { continue; }

            // Compute actual rollout values
            let mut rollout_values: Vec<(usize, f64)> = Vec::new();
            for &pos in &empty {
                let mut temp_plateau = plateau.clone();
                temp_plateau.tiles[pos] = test_tile;
                let temp_deck = replace_tile_in_deck(&deck, &test_tile);

                let mut total = 0.0;
                for _ in 0..30 {
                    total += simulate_games_smart(temp_plateau.clone(), temp_deck.clone(), None) as f64;
                }
                rollout_values.push((pos, total / 30.0));
            }

            // Sort by Q-net and by rollout
            let mut qnet_ranking: Vec<(usize, f64)> = empty.iter()
                .map(|&p| (p, qvalues[p]))
                .collect();
            qnet_ranking.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            rollout_values.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            // Check if Q-net top-1 is in rollout top-3
            let qnet_best = qnet_ranking[0].0;
            let rollout_top3: Vec<usize> = rollout_values.iter().take(3).map(|(p, _)| *p).collect();
            let rollout_best = rollout_values[0].0;

            if qnet_best == rollout_best {
                correct_top1 += 1;
            }
            if rollout_top3.contains(&qnet_best) {
                correct_top3 += 1;
            }
            total_tests += 1;

            log::info!("   State {}: Q-net best={}, Rollout best={}, Q-net in top3={}",
                i, qnet_best, rollout_best, rollout_top3.contains(&qnet_best));
        }

        log::info!("   ACCURACY: Top-1={}/{} ({:.0}%), In-Top-3={}/{} ({:.0}%)",
            correct_top1, total_tests, correct_top1 as f64 / total_tests as f64 * 100.0,
            correct_top3, total_tests, correct_top3 as f64 / total_tests as f64 * 100.0);

        // Random baseline: Top-1 accuracy with N positions is 1/N ≈ 5-10%
        // In-Top-3 accuracy is 3/N ≈ 15-25%
    }

    let mut rng = rand::rngs::StdRng::seed_from_u64(args.seed);

    // Run benchmarks based on strategy
    let strategies: Vec<&str> = if args.strategy == "all" {
        vec!["pure", "prune", "progressive", "leaf", "combined", "softmax"]
    } else {
        vec![args.strategy.as_str()]
    };

    let mut results: HashMap<String, (f64, f64, Vec<i32>)> = HashMap::new();

    for strategy in &strategies {
        log::info!("\n📊 Running strategy: {}", strategy);

        let mut scores = Vec::new();

        for game_idx in 0..args.games {
            // Sample tiles for this game
            let tiles = sample_tiles(&mut rng, 19);

            let score = match *strategy {
                "pure" => play_game_pure(&tiles, args.simulations),
                "prune" => play_game_prune(&tiles, args.simulations, &qnet, args.top_k),
                "progressive" => play_game_progressive(&tiles, args.simulations, &qnet, args.bias_coef),
                "leaf" => play_game_leaf_init(&tiles, args.simulations, &qnet),
                "combined" => play_game_combined(&tiles, args.simulations, &qnet, args.top_k, args.bias_coef),
                "softmax" => play_game_softmax(&tiles, args.simulations, &qnet),
                _ => play_game_pure(&tiles, args.simulations),
            };

            scores.push(score);

            if (game_idx + 1) % 10 == 0 {
                log::info!("  Progress: {}/{}", game_idx + 1, args.games);
            }
        }

        let mean = scores.iter().sum::<i32>() as f64 / args.games as f64;
        let std = std_dev(&scores);
        results.insert(strategy.to_string(), (mean, std, scores));
    }

    // Print results
    println!("\n{}", "=".repeat(60));
    println!("     Q-NET AS PRIOR POLICY - COMPARISON RESULTS");
    println!("{}", "=".repeat(60));
    println!("Games: {}, Simulations: {}, Top-K: {}, Bias: {:.1}",
        args.games, args.simulations, args.top_k, args.bias_coef);
    println!();

    let pure_mean = results.get("pure").map(|(m, _, _)| *m).unwrap_or(0.0);

    for strategy in &["pure", "prune", "progressive", "leaf", "combined", "softmax"] {
        if let Some((mean, std, scores)) = results.get(*strategy) {
            let delta = mean - pure_mean;
            let wins = if *strategy != "pure" && results.contains_key("pure") {
                let pure_scores = &results.get("pure").unwrap().2;
                scores.iter().zip(pure_scores.iter())
                    .filter(|(s, p)| s > p).count()
            } else {
                0
            };

            let delta_str = if *strategy == "pure" {
                "  -  ".to_string()
            } else {
                format!("{:+.2}", delta)
            };

            let win_str = if *strategy == "pure" {
                "  -  ".to_string()
            } else {
                format!("{}/{} ({:.0}%)", wins, args.games, wins as f64 / args.games as f64 * 100.0)
            };

            println!("{:12} : mean={:>6.2}, std={:>5.2}, delta={}, wins={}",
                strategy, mean, std, delta_str, win_str);
        }
    }
    println!("{}\n", "=".repeat(60));

    // Interpretation
    println!("📈 INTERPRETATION:");
    if let (Some((_, _, _)), Some((prune_mean, _, _))) = (results.get("pure"), results.get("prune")) {
        if *prune_mean > pure_mean + 2.0 {
            println!("  ✅ PRUNE works! Q-net successfully identifies good positions");
        } else if *prune_mean < pure_mean - 2.0 {
            println!("  ❌ PRUNE hurts - Q-net may be pruning good positions");
        } else {
            println!("  ➖ PRUNE neutral - Q-net doesn't help/hurt action selection");
        }
    }

    if let (Some((_, _, _)), Some((prog_mean, _, _))) = (results.get("pure"), results.get("progressive")) {
        if *prog_mean > pure_mean + 2.0 {
            println!("  ✅ PROGRESSIVE works! Q-net priors improve early exploration");
        } else if *prog_mean < pure_mean - 2.0 {
            println!("  ❌ PROGRESSIVE hurts - Q-net biases search poorly");
        } else {
            println!("  ➖ PROGRESSIVE neutral - bias doesn't help");
        }
    }

    Ok(())
}

// =============================================================================
// STRATEGY 1: PURE MCTS (baseline)
// =============================================================================
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

fn find_best_position_pure(plateau: &Plateau, deck: &Deck, tile: Tile, num_sims: usize) -> usize {
    let empty_positions: Vec<usize> = (0..19)
        .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
        .collect();

    let mut best_pos = empty_positions[0];
    let mut best_value = f64::NEG_INFINITY;

    for &pos in &empty_positions {
        let mut temp_plateau = plateau.clone();
        temp_plateau.tiles[pos] = tile;
        let temp_deck = replace_tile_in_deck(deck, &tile);

        let mut total = 0.0;
        for _ in 0..num_sims {
            total += simulate_games_smart(temp_plateau.clone(), temp_deck.clone(), None) as f64;
        }
        let avg = total / num_sims as f64;

        if avg > best_value {
            best_value = avg;
            best_pos = pos;
        }
    }

    best_pos
}

// =============================================================================
// STRATEGY 2: ACTION PRUNING - Only explore top-K positions by Q-net
// =============================================================================
fn play_game_prune(tiles: &[Tile], num_sims: usize, qnet: &QValueNet, top_k: usize) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for tile in tiles {
        let best_pos = find_best_position_prune(&plateau, &deck, *tile, num_sims, qnet, top_k);
        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn find_best_position_prune(
    plateau: &Plateau,
    deck: &Deck,
    tile: Tile,
    num_sims: usize,
    qnet: &QValueNet,
    top_k: usize,
) -> usize {
    let empty_positions: Vec<usize> = (0..19)
        .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
        .collect();

    if empty_positions.len() <= top_k {
        // Not enough positions to prune, fall back to pure
        return find_best_position_pure(plateau, deck, tile, num_sims);
    }

    // Get Q-values and sort positions by Q-value (descending)
    let qvalues = qnet.predict_qvalues(&plateau.tiles, &tile);

    let mut scored_positions: Vec<(usize, f64)> = empty_positions
        .iter()
        .map(|&pos| (pos, qvalues[pos]))
        .collect();

    scored_positions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Only explore top-K positions
    let top_positions: Vec<usize> = scored_positions.iter()
        .take(top_k)
        .map(|(pos, _)| *pos)
        .collect();

    // Run MCTS only on top-K positions
    let mut best_pos = top_positions[0];
    let mut best_value = f64::NEG_INFINITY;

    for &pos in &top_positions {
        let mut temp_plateau = plateau.clone();
        temp_plateau.tiles[pos] = tile;
        let temp_deck = replace_tile_in_deck(deck, &tile);

        let mut total = 0.0;
        for _ in 0..num_sims {
            total += simulate_games_smart(temp_plateau.clone(), temp_deck.clone(), None) as f64;
        }
        let avg = total / num_sims as f64;

        if avg > best_value {
            best_value = avg;
            best_pos = pos;
        }
    }

    best_pos
}

// =============================================================================
// STRATEGY 3: PROGRESSIVE BIAS - Q-net influence decreases with visits
// Formula: UCB = Q_rollout + (bias_coef * Q_net) / (1 + visits)
// =============================================================================
fn play_game_progressive(tiles: &[Tile], num_sims: usize, qnet: &QValueNet, bias_coef: f64) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for tile in tiles {
        let best_pos = find_best_position_progressive(&plateau, &deck, *tile, num_sims, qnet, bias_coef);
        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn find_best_position_progressive(
    plateau: &Plateau,
    deck: &Deck,
    tile: Tile,
    num_sims: usize,
    qnet: &QValueNet,
    bias_coef: f64,
) -> usize {
    let empty_positions: Vec<usize> = (0..19)
        .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
        .collect();

    let qvalues = qnet.predict_qvalues(&plateau.tiles, &tile);

    // Initialize tracking
    let mut visit_counts: HashMap<usize, usize> = HashMap::new();
    let mut total_scores: HashMap<usize, f64> = HashMap::new();

    for &pos in &empty_positions {
        visit_counts.insert(pos, 0);
        total_scores.insert(pos, 0.0);
    }

    // Distribute simulations using progressive bias for selection
    for _ in 0..num_sims {
        // Select position using progressive bias UCB
        let mut best_ucb = f64::NEG_INFINITY;
        let mut selected_pos = empty_positions[0];

        for &pos in &empty_positions {
            let visits = visit_counts[&pos];
            let q_rollout = if visits > 0 {
                total_scores[&pos] / visits as f64
            } else {
                100.0  // Optimistic initial value
            };

            // Progressive bias: Q-net influence decreases with visits
            // This is exactly how AlphaGo Zero uses the policy prior!
            let q_net_bias = (bias_coef * qvalues[pos] * 200.0) / (1.0 + visits as f64);

            // Exploration bonus (UCB1-style)
            let total_visits: usize = visit_counts.values().sum();
            let exploration = if visits > 0 {
                2.0 * ((total_visits as f64).ln() / visits as f64).sqrt()
            } else {
                f64::INFINITY  // Must visit unvisited positions
            };

            let ucb = q_rollout + q_net_bias + exploration;

            if ucb > best_ucb {
                best_ucb = ucb;
                selected_pos = pos;
            }
        }

        // Simulate selected position
        let mut temp_plateau = plateau.clone();
        temp_plateau.tiles[selected_pos] = tile;
        let temp_deck = replace_tile_in_deck(deck, &tile);

        let score = simulate_games_smart(temp_plateau, temp_deck, None) as f64;

        *visit_counts.get_mut(&selected_pos).unwrap() += 1;
        *total_scores.get_mut(&selected_pos).unwrap() += score;
    }

    // Select best position by visit count (most explored = most promising)
    empty_positions.into_iter()
        .max_by_key(|&pos| visit_counts[&pos])
        .unwrap_or(0)
}

// =============================================================================
// STRATEGY 4: LEAF INITIALIZATION - Use Q(s,a) as initial value
// =============================================================================
fn play_game_leaf_init(tiles: &[Tile], num_sims: usize, qnet: &QValueNet) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for tile in tiles {
        let best_pos = find_best_position_leaf_init(&plateau, &deck, *tile, num_sims, qnet);
        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn find_best_position_leaf_init(
    plateau: &Plateau,
    deck: &Deck,
    tile: Tile,
    num_sims: usize,
    qnet: &QValueNet,
) -> usize {
    let empty_positions: Vec<usize> = (0..19)
        .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
        .collect();

    let qvalues = qnet.predict_qvalues(&plateau.tiles, &tile);

    // Initialize with Q-net predictions (as if 1 virtual visit)
    let mut visit_counts: HashMap<usize, usize> = HashMap::new();
    let mut total_scores: HashMap<usize, f64> = HashMap::new();

    for &pos in &empty_positions {
        // Start with Q-net value as "virtual" first sample
        visit_counts.insert(pos, 1);
        total_scores.insert(pos, qvalues[pos] * 200.0);  // Scale to score range
    }

    // Run rollouts and add to existing Q-net initialization
    let sims_per_pos = num_sims / empty_positions.len().max(1);

    for &pos in &empty_positions {
        let mut temp_plateau = plateau.clone();
        temp_plateau.tiles[pos] = tile;
        let temp_deck = replace_tile_in_deck(deck, &tile);

        for _ in 0..sims_per_pos {
            let score = simulate_games_smart(temp_plateau.clone(), temp_deck.clone(), None) as f64;
            *visit_counts.get_mut(&pos).unwrap() += 1;
            *total_scores.get_mut(&pos).unwrap() += score;
        }
    }

    // Select by average score (combines Q-net init + rollouts)
    empty_positions.into_iter()
        .max_by(|&a, &b| {
            let avg_a = total_scores[&a] / visit_counts[&a] as f64;
            let avg_b = total_scores[&b] / visit_counts[&b] as f64;
            avg_a.partial_cmp(&avg_b).unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(0)
}

// =============================================================================
// STRATEGY 5: COMBINED - Prune + Progressive Bias + Leaf Init
// =============================================================================
fn play_game_combined(
    tiles: &[Tile],
    num_sims: usize,
    qnet: &QValueNet,
    top_k: usize,
    bias_coef: f64,
) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for tile in tiles {
        let best_pos = find_best_position_combined(&plateau, &deck, *tile, num_sims, qnet, top_k, bias_coef);
        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn find_best_position_combined(
    plateau: &Plateau,
    deck: &Deck,
    tile: Tile,
    num_sims: usize,
    qnet: &QValueNet,
    top_k: usize,
    bias_coef: f64,
) -> usize {
    let empty_positions: Vec<usize> = (0..19)
        .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
        .collect();

    let qvalues = qnet.predict_qvalues(&plateau.tiles, &tile);

    // Step 1: PRUNE - Sort and keep top-K positions
    let mut scored_positions: Vec<(usize, f64)> = empty_positions
        .iter()
        .map(|&pos| (pos, qvalues[pos]))
        .collect();

    scored_positions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let k = top_k.min(empty_positions.len());
    let candidate_positions: Vec<usize> = scored_positions.iter()
        .take(k)
        .map(|(pos, _)| *pos)
        .collect();

    // Step 2: LEAF INIT - Initialize with Q-net values
    let mut visit_counts: HashMap<usize, usize> = HashMap::new();
    let mut total_scores: HashMap<usize, f64> = HashMap::new();

    for &pos in &candidate_positions {
        visit_counts.insert(pos, 1);
        total_scores.insert(pos, qvalues[pos] * 200.0);
    }

    // Step 3: PROGRESSIVE BIAS - Run MCTS with decreasing Q-net influence
    for _ in 0..num_sims {
        let mut best_ucb = f64::NEG_INFINITY;
        let mut selected_pos = candidate_positions[0];

        for &pos in &candidate_positions {
            let visits = visit_counts[&pos];
            let q_rollout = total_scores[&pos] / visits as f64;

            // Progressive bias decreases with visits
            let q_net_bias = (bias_coef * qvalues[pos] * 200.0) / (1.0 + visits as f64);

            let total_visits: usize = visit_counts.values().sum();
            let exploration = 2.0 * ((total_visits as f64).ln() / visits as f64).sqrt();

            let ucb = q_rollout + q_net_bias + exploration;

            if ucb > best_ucb {
                best_ucb = ucb;
                selected_pos = pos;
            }
        }

        // Simulate
        let mut temp_plateau = plateau.clone();
        temp_plateau.tiles[selected_pos] = tile;
        let temp_deck = replace_tile_in_deck(deck, &tile);

        let score = simulate_games_smart(temp_plateau, temp_deck, None) as f64;

        *visit_counts.get_mut(&selected_pos).unwrap() += 1;
        *total_scores.get_mut(&selected_pos).unwrap() += score;
    }

    // Select by visit count
    candidate_positions.into_iter()
        .max_by_key(|&pos| visit_counts[&pos])
        .unwrap_or(0)
}

// =============================================================================
// STRATEGY 6: SOFTMAX - Two-phase: Q-net for initial estimate, rollouts to refine
// Phase 1: Get Q-net ranking to identify top candidates
// Phase 2: Run rollouts ONLY on top candidates (fewer positions = more sims each)
// =============================================================================
fn play_game_softmax(tiles: &[Tile], num_sims: usize, qnet: &QValueNet) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for tile in tiles {
        let best_pos = find_best_position_softmax(&plateau, &deck, *tile, num_sims, qnet);
        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn find_best_position_softmax(
    plateau: &Plateau,
    deck: &Deck,
    tile: Tile,
    num_sims: usize,
    qnet: &QValueNet,
) -> usize {
    let empty_positions: Vec<usize> = (0..19)
        .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
        .collect();

    if empty_positions.len() <= 2 {
        // Too few positions, just do pure MCTS
        return find_best_position_pure(plateau, deck, tile, num_sims);
    }

    // Phase 1: Quick Q-net evaluation (FREE - no rollouts)
    let qvalues = qnet.predict_qvalues(&plateau.tiles, &tile);

    // Sort positions by Q-value (descending)
    let mut scored: Vec<(usize, f64)> = empty_positions.iter()
        .map(|&pos| (pos, qvalues[pos]))
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Phase 2: Run rollouts ONLY on top 50% of positions
    // This gives each candidate 2x more simulations!
    let num_candidates = (empty_positions.len() / 2).max(3).min(empty_positions.len());
    let candidates: Vec<usize> = scored.iter().take(num_candidates).map(|(pos, _)| *pos).collect();

    let sims_per_candidate = num_sims / candidates.len();

    let mut best_pos = candidates[0];
    let mut best_avg = f64::NEG_INFINITY;

    for &pos in &candidates {
        let mut temp_plateau = plateau.clone();
        temp_plateau.tiles[pos] = tile;
        let temp_deck = replace_tile_in_deck(deck, &tile);

        let mut total = 0.0;
        for _ in 0..sims_per_candidate {
            total += simulate_games_smart(temp_plateau.clone(), temp_deck.clone(), None) as f64;
        }
        let avg = total / sims_per_candidate as f64;

        if avg > best_avg {
            best_avg = avg;
            best_pos = pos;
        }
    }

    best_pos
}

// =============================================================================
// UTILITIES
// =============================================================================

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
