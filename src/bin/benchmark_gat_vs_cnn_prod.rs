//! Benchmark: GAT Elite150 vs CNN Q-net (Production)
//!
//! Compare the GAT model trained on elite games against the production CNN + Q-net.
//!
//! Usage: cargo run --release --bin benchmark_gat_vs_cnn_prod -- --games 100

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;
use tch::{nn, nn::OptimizerConfig, Device, IndexOp, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::deck::Deck;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_pure;
use take_it_easy::neural::gat::GATPolicyNet;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::neural::{NeuralConfig, NeuralManager, QNetManager};
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "benchmark_gat_vs_cnn_prod")]
struct Args {
    /// Number of games to play
    #[arg(long, default_value_t = 100)]
    games: usize,

    /// MCTS simulations per move
    #[arg(long, default_value_t = 200)]
    simulations: usize,

    /// Top-K positions to consider for pruning
    #[arg(long, default_value_t = 6)]
    top_k: usize,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Minimum score for training data
    #[arg(long, default_value_t = 150)]
    min_score: i32,

    /// Training epochs for GAT
    #[arg(long, default_value_t = 30)]
    epochs: usize,
}

/// Training sample from CSV
#[derive(Clone)]
struct Sample {
    plateau: [i32; 19],
    tile: (i32, i32, i32),
    position: usize,
    turn: usize,
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     Benchmark: GAT Elite vs CNN Q-net (Production)          ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("Config:");
    println!("  Games:       {}", args.games);
    println!("  Simulations: {}", args.simulations);
    println!("  Top-K:       {}", args.top_k);
    println!("  Min score:   {}", args.min_score);
    println!("  Seed:        {}", args.seed);

    // Train GAT from elite data
    println!("\n🔷 Training GAT from elite games (score >= {})...", args.min_score);
    let train_start = Instant::now();
    let (vs_gat, gat_policy) = train_gat_from_elite(args.min_score, args.epochs, args.seed);
    println!("   ✅ GAT trained in {:.1}s", train_start.elapsed().as_secs_f32());
    let _ = vs_gat; // Keep varstore alive

    // Load CNN + Q-net
    println!("\n🔶 Loading CNN + Q-net (production)...");
    let neural_config = NeuralConfig::default();
    let neural_manager = match NeuralManager::with_config(neural_config) {
        Ok(m) => {
            println!("   ✅ CNN policy/value loaded");
            Some(m)
        }
        Err(e) => {
            println!("   ⚠️ Could not load CNN: {}", e);
            None
        }
    };

    let qnet_manager = match QNetManager::new("model_weights/qvalue_net.params") {
        Ok(m) => {
            println!("   ✅ Q-net loaded");
            Some(m)
        }
        Err(e) => {
            println!("   ⚠️ Could not load Q-net: {}", e);
            None
        }
    };

    // Run benchmark
    println!("\n📊 Running benchmark on {} games...\n", args.games);

    let mut rng = StdRng::seed_from_u64(args.seed);

    let mut greedy_scores = Vec::new();
    let mut pure_mcts_scores = Vec::new();
    let mut gat_policy_scores = Vec::new();
    let mut gat_mcts_scores = Vec::new();
    let mut cnn_qnet_scores = Vec::new();

    let start = Instant::now();

    for game_idx in 0..args.games {
        let tiles = sample_tiles(&mut rng, 19);

        // Greedy baseline
        greedy_scores.push(play_greedy(&tiles));

        // Pure MCTS
        pure_mcts_scores.push(play_pure_mcts(&tiles, args.simulations));

        // GAT policy only (no MCTS)
        gat_policy_scores.push(play_gat_policy(&tiles, &gat_policy));

        // GAT + MCTS
        gat_mcts_scores.push(play_gat_mcts(&tiles, args.simulations, &gat_policy, args.top_k));

        // CNN + Q-net (production)
        if let (Some(ref nm), Some(ref qm)) = (&neural_manager, &qnet_manager) {
            cnn_qnet_scores.push(play_cnn_qnet(&tiles, args.simulations, nm, qm, args.top_k));
        }

        if (game_idx + 1) % 10 == 0 || game_idx == args.games - 1 {
            let elapsed = start.elapsed().as_secs_f32();
            let eta = elapsed / (game_idx + 1) as f32 * (args.games - game_idx - 1) as f32;
            print!("   Game {}/{} ({:.1}s elapsed, ETA {:.1}s)...\r",
                   game_idx + 1, args.games, elapsed, eta);
        }
    }
    println!();

    let total_time = start.elapsed().as_secs_f32();

    // Calculate statistics
    let greedy_stats = calc_stats(&greedy_scores);
    let pure_mcts_stats = calc_stats(&pure_mcts_scores);
    let gat_policy_stats = calc_stats(&gat_policy_scores);
    let gat_mcts_stats = calc_stats(&gat_mcts_scores);

    // Results
    println!("\n╔════════════════════════════════════════════════════════════════════════════╗");
    println!("║                              RESULTS                                       ║");
    println!("╠════════════════════════════════════════════════════════════════════════════╣");
    println!("║  Method                │   Mean  │  Std  │   Min │   Max │ >=100 │ >=150  ║");
    println!("╠════════════════════════════════════════════════════════════════════════════╣");

    print_row("Greedy", &greedy_stats, &greedy_scores);
    print_row(&format!("Pure MCTS ({} sim)", args.simulations), &pure_mcts_stats, &pure_mcts_scores);
    print_row("GAT Policy (no MCTS)", &gat_policy_stats, &gat_policy_scores);
    print_row("GAT + MCTS", &gat_mcts_stats, &gat_mcts_scores);

    if !cnn_qnet_scores.is_empty() {
        let cnn_stats = calc_stats(&cnn_qnet_scores);
        print_row("CNN + Q-net (prod)", &cnn_stats, &cnn_qnet_scores);
    }

    println!("╚════════════════════════════════════════════════════════════════════════════╝");

    // Head-to-head comparison
    if !cnn_qnet_scores.is_empty() {
        println!("\n📈 HEAD-TO-HEAD COMPARISON:");
        println!("   ────────────────────────────────────────────────────────");

        let cnn_mean = mean(&cnn_qnet_scores);
        let gat_mcts_mean = gat_mcts_stats.0;
        let gat_policy_mean = gat_policy_stats.0;

        println!("   GAT Policy vs CNN Q-net: {:>+.2} pts", gat_policy_mean - cnn_mean);
        println!("   GAT + MCTS vs CNN Q-net: {:>+.2} pts", gat_mcts_mean - cnn_mean);
        println!("   GAT + MCTS vs Pure MCTS: {:>+.2} pts", gat_mcts_mean - pure_mcts_stats.0);

        // Win/Loss count
        let mut gat_wins = 0;
        let mut cnn_wins = 0;
        let mut ties = 0;
        for i in 0..args.games {
            if gat_mcts_scores[i] > cnn_qnet_scores[i] {
                gat_wins += 1;
            } else if gat_mcts_scores[i] < cnn_qnet_scores[i] {
                cnn_wins += 1;
            } else {
                ties += 1;
            }
        }

        println!("\n   Win/Loss (GAT+MCTS vs CNN+Qnet):");
        println!("   GAT wins: {} ({:.1}%)", gat_wins, gat_wins as f64 / args.games as f64 * 100.0);
        println!("   CNN wins: {} ({:.1}%)", cnn_wins, cnn_wins as f64 / args.games as f64 * 100.0);
        println!("   Ties:     {} ({:.1}%)", ties, ties as f64 / args.games as f64 * 100.0);

        if gat_mcts_mean > cnn_mean + 2.0 {
            println!("\n   🏆 GAT + MCTS BEATS CNN + Q-net!");
        } else if gat_mcts_mean > cnn_mean - 2.0 {
            println!("\n   ⚖️ GAT + MCTS matches CNN + Q-net (within margin)");
        } else {
            println!("\n   📉 CNN + Q-net still ahead");
        }
    }

    println!("\n⏱️ Total benchmark time: {:.1}s ({:.2}s/game)", total_time, total_time / args.games as f32);
}

fn calc_stats(scores: &[i32]) -> (f64, f64, i32, i32) {
    let mean = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
    let variance = scores.iter()
        .map(|&s| (s as f64 - mean).powi(2))
        .sum::<f64>() / scores.len() as f64;
    let std = variance.sqrt();
    let min = *scores.iter().min().unwrap_or(&0);
    let max = *scores.iter().max().unwrap_or(&0);
    (mean, std, min, max)
}

fn print_row(name: &str, stats: &(f64, f64, i32, i32), scores: &[i32]) {
    let above_100 = scores.iter().filter(|&&s| s >= 100).count();
    let above_150 = scores.iter().filter(|&&s| s >= 150).count();
    let pct_100 = above_100 as f64 / scores.len() as f64 * 100.0;
    let pct_150 = above_150 as f64 / scores.len() as f64 * 100.0;

    println!("║  {:22}│ {:>7.2} │ {:>5.1} │ {:>5} │ {:>5} │ {:>4.0}% │ {:>4.0}%  ║",
             name, stats.0, stats.1, stats.2, stats.3, pct_100, pct_150);
}

fn mean(values: &[i32]) -> f64 {
    if values.is_empty() { return 0.0; }
    values.iter().sum::<i32>() as f64 / values.len() as f64
}

fn sample_tiles(rng: &mut StdRng, count: usize) -> Vec<Tile> {
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

fn play_greedy(tiles: &[Tile]) -> i32 {
    let mut plateau = create_plateau_empty();
    for tile in tiles {
        let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
        if avail.is_empty() { break; }
        let pos = avail.iter().copied().max_by_key(|&pos| {
            let mut test = plateau.clone();
            test.tiles[pos] = *tile;
            result(&test)
        }).unwrap_or(avail[0]);
        plateau.tiles[pos] = *tile;
    }
    result(&plateau)
}

fn play_pure_mcts(tiles: &[Tile], num_sims: usize) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let mcts_result = mcts_find_best_position_for_tile_pure(
            &mut plateau,
            &mut deck,
            *tile,
            num_sims,
            turn,
            19,
            None,
        );
        plateau.tiles[mcts_result.best_position] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn play_gat_policy(tiles: &[Tile], policy: &GATPolicyNet) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
        if avail.is_empty() { break; }

        let features = convert_plateau_for_gat_47ch(&plateau, tile, &deck, turn, 19);
        let logits = policy.forward(&features.unsqueeze(0), false);

        let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
        for &pos in &avail {
            let _ = mask.i(pos as i64).fill_(0.0);
        }
        let best: i64 = (logits.squeeze_dim(0) + mask).argmax(0, false).try_into().unwrap();

        plateau.tiles[best as usize] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn play_gat_mcts(tiles: &[Tile], num_sims: usize, policy: &GATPolicyNet, top_k: usize) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
        if avail.is_empty() { break; }

        // Use GAT policy for pruning when many positions available
        let should_prune = avail.len() > top_k + 2;

        let best_pos = if should_prune {
            let features = convert_plateau_for_gat_47ch(&plateau, tile, &deck, turn, 19);
            let logits = policy.forward(&features.unsqueeze(0), false).squeeze_dim(0);

            let mut scored: Vec<(usize, f64)> = avail.iter()
                .map(|&pos| (pos, logits.double_value(&[pos as i64])))
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let top_positions: Vec<usize> = scored.iter().take(top_k).map(|(pos, _)| *pos).collect();

            // Run MCTS on top positions
            let sims_per_pos = num_sims / top_positions.len().max(1);
            let mut best = top_positions[0];
            let mut best_score = f64::NEG_INFINITY;

            for &pos in &top_positions {
                let mut temp_plateau = plateau.clone();
                temp_plateau.tiles[pos] = *tile;
                let temp_deck = replace_tile_in_deck(&deck, tile);

                let mut total = 0.0;
                for _ in 0..sims_per_pos {
                    total += simulate_random_game(&temp_plateau, &temp_deck.clone()) as f64;
                }
                let avg = total / sims_per_pos as f64;

                if avg > best_score {
                    best_score = avg;
                    best = pos;
                }
            }
            best
        } else {
            // Full MCTS when few positions
            let mcts_result = mcts_find_best_position_for_tile_pure(
                &mut plateau.clone(),
                &mut deck.clone(),
                *tile,
                num_sims,
                turn,
                19,
                None,
            );
            mcts_result.best_position
        };

        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn play_cnn_qnet(
    tiles: &[Tile],
    num_sims: usize,
    neural_manager: &NeuralManager,
    qnet_manager: &QNetManager,
    top_k: usize,
) -> i32 {
    use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_with_qnet;

    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    let policy_net = neural_manager.policy_net();
    let value_net = neural_manager.value_net();
    let qvalue_net = qnet_manager.net();

    for (turn, tile) in tiles.iter().enumerate() {
        let mcts_result = mcts_find_best_position_for_tile_with_qnet(
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
        plateau.tiles[mcts_result.best_position] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn simulate_random_game(plateau: &Plateau, deck: &Deck) -> i32 {
    let mut rng = rand::rng();
    let mut plateau = plateau.clone();
    let mut deck = deck.clone();

    loop {
        let tiles = get_available_tiles(&deck);
        if tiles.is_empty() { break; }
        let tile = *tiles.choose(&mut rng).unwrap();

        let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
        if avail.is_empty() { break; }
        let pos = *avail.choose(&mut rng).unwrap();

        plateau.tiles[pos] = tile;
        deck = replace_tile_in_deck(&deck, &tile);
    }

    result(&plateau)
}

/// Train GAT from elite game data (CSV files)
fn train_gat_from_elite(min_score: i32, epochs: usize, seed: u64) -> (nn::VarStore, GATPolicyNet) {
    let samples = load_all_csv("data", min_score);
    println!("   Loaded {} samples from games with score >= {}", samples.len(), min_score);

    if samples.is_empty() {
        println!("   ⚠️ No samples found, using random initialization");
        let vs = nn::VarStore::new(Device::Cpu);
        let policy = GATPolicyNet::new(&vs, 47, &[128, 128], 4, 0.1);
        return (vs, policy);
    }

    let vs = nn::VarStore::new(Device::Cpu);
    let policy = GATPolicyNet::new(&vs, 47, &[128, 128], 4, 0.1);
    let mut opt = nn::Adam::default().build(&vs, 0.001).unwrap();

    let mut rng = StdRng::seed_from_u64(seed);
    let batch_size = 64;
    let n_batches = samples.len() / batch_size;

    for epoch in 0..epochs {
        let mut indices: Vec<usize> = (0..samples.len()).collect();
        indices.shuffle(&mut rng);

        let mut total_loss = 0.0;
        let mut total_correct = 0usize;

        for batch_i in 0..n_batches {
            let batch_indices: Vec<usize> = indices[batch_i * batch_size..(batch_i + 1) * batch_size].to_vec();

            let (features, targets, masks) = prepare_batch(&samples, &batch_indices);

            let logits = policy.forward(&features, true);
            let masked_logits = logits + &masks;
            let log_probs = masked_logits.log_softmax(-1, Kind::Float);
            let loss = -log_probs
                .gather(1, &targets.unsqueeze(1), false)
                .squeeze_dim(1)
                .mean(Kind::Float);

            opt.backward_step(&loss);
            total_loss += f64::try_from(&loss).unwrap();

            let preds = masked_logits.argmax(-1, false);
            let correct: i64 = preds.eq_tensor(&targets).sum(Kind::Int64).int64_value(&[]);
            total_correct += correct as usize;
        }

        if (epoch + 1) % 10 == 0 || epoch == epochs - 1 {
            let acc = total_correct as f64 / (n_batches * batch_size) as f64 * 100.0;
            println!("   Epoch {}/{}: loss={:.4}, acc={:.1}%", epoch + 1, epochs, total_loss / n_batches as f64, acc);
        }
    }

    (vs, policy)
}

/// Load all CSV files from directory
fn load_all_csv(dir: &str, min_score: i32) -> Vec<Sample> {
    let mut samples = Vec::new();

    let path = Path::new(dir);
    if !path.exists() {
        return samples;
    }

    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let file_path = entry.path();
        if file_path.extension().map_or(false, |e| e == "csv") {
            let file_samples = load_csv(&file_path, min_score);
            samples.extend(file_samples);
        }
    }

    samples
}

/// Load samples from a single CSV file
fn load_csv(path: &Path, min_score: i32) -> Vec<Sample> {
    let mut samples = Vec::new();

    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return samples,
    };

    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    // Skip header
    let _ = lines.next();

    for line in lines {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < 28 {
            continue;
        }

        let final_score: i32 = match fields[26].parse() {
            Ok(s) => s,
            Err(_) => continue,
        };

        if final_score < min_score {
            continue;
        }

        let turn: usize = match fields[1].parse() {
            Ok(t) => t,
            Err(_) => continue,
        };

        let mut plateau = [0i32; 19];
        for i in 0..19 {
            plateau[i] = fields[3 + i].parse().unwrap_or(0);
        }

        let tile = (
            fields[22].parse().unwrap_or(0),
            fields[23].parse().unwrap_or(0),
            fields[24].parse().unwrap_or(0),
        );

        let position: usize = match fields[25].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        samples.push(Sample {
            plateau,
            tile,
            position,
            turn,
        });
    }

    samples
}

/// Decode tile from encoded integer
fn decode_tile(encoded: i32) -> Tile {
    if encoded == 0 {
        return Tile(0, 0, 0);
    }
    let t0 = encoded / 100;
    let t1 = (encoded / 10) % 10;
    let t2 = encoded % 10;
    Tile(t0, t1, t2)
}

/// Convert sample to GAT features
fn sample_to_features(sample: &Sample) -> Tensor {
    let mut plateau = Plateau {
        tiles: vec![Tile(0, 0, 0); 19],
    };
    for i in 0..19 {
        plateau.tiles[i] = decode_tile(sample.plateau[i]);
    }

    let tile = Tile(sample.tile.0, sample.tile.1, sample.tile.2);
    let deck = create_deck();

    convert_plateau_for_gat_47ch(&plateau, &tile, &deck, sample.turn, 19)
}

/// Get mask for available positions
fn get_available_mask(sample: &Sample) -> Tensor {
    let mut mask = vec![f64::NEG_INFINITY; 19];
    for i in 0..19 {
        if sample.plateau[i] == 0 {
            mask[i] = 0.0;
        }
    }
    Tensor::from_slice(&mask)
}

/// Prepare a batch of samples
fn prepare_batch(samples: &[Sample], indices: &[usize]) -> (Tensor, Tensor, Tensor) {
    let features: Vec<Tensor> = indices.iter()
        .map(|&i| sample_to_features(&samples[i]))
        .collect();

    let targets: Vec<i64> = indices.iter()
        .map(|&i| samples[i].position as i64)
        .collect();

    let masks: Vec<Tensor> = indices.iter()
        .map(|&i| get_available_mask(&samples[i]))
        .collect();

    (
        Tensor::stack(&features, 0),
        Tensor::from_slice(&targets),
        Tensor::stack(&masks, 0),
    )
}
