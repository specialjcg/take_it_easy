//! Train GAT Supervised then compare with CNN + MCTS + Q-Net
//!
//! All-in-one script that:
//! 1. Trains GAT on high-score games (>= 140 pts)
//! 2. Compares with CNN + Q-Net MCTS
//!
//! Usage: cargo run --release --bin train_and_compare_gat -- --epochs 50 --games 100

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::collections::HashMap;
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
#[command(name = "train_and_compare_gat")]
struct Args {
    /// Minimum score for training data
    #[arg(long, default_value_t = 140)]
    min_score: i32,

    /// Training epochs
    #[arg(long, default_value_t = 50)]
    epochs: usize,

    /// Batch size
    #[arg(long, default_value_t = 64)]
    batch_size: usize,

    /// Learning rate
    #[arg(long, default_value_t = 0.001)]
    lr: f64,

    /// Evaluation games
    #[arg(long, default_value_t = 100)]
    games: usize,

    /// MCTS simulations
    #[arg(long, default_value_t = 200)]
    simulations: usize,

    /// Top-K for pruning
    #[arg(long, default_value_t = 6)]
    top_k: usize,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

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
    println!("║    Train GAT Supervised & Compare with CNN + Q-Net          ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // ========== PHASE 1: Load training data ==========
    println!("📂 Loading training data (score >= {})...", args.min_score);
    let samples = load_all_csv("data", args.min_score);
    println!("   Loaded {} samples", samples.len());

    if samples.is_empty() {
        println!("❌ No samples found!");
        return;
    }

    // ========== PHASE 2: Train GAT ==========
    println!("\n🔷 Training GAT Supervised ({} epochs)...\n", args.epochs);
    let mut rng = StdRng::seed_from_u64(args.seed);

    let device = Device::Cpu;
    let vs = nn::VarStore::new(device);
    let gat_policy = GATPolicyNet::new(&vs, 47, &[128, 128], 4, 0.1);
    let mut opt = nn::Adam::default().build(&vs, args.lr).unwrap();

    // Split data
    let mut indices: Vec<usize> = (0..samples.len()).collect();
    indices.shuffle(&mut rng);
    let val_size = (samples.len() as f64 * 0.1) as usize;
    let val_indices: Vec<usize> = indices[..val_size].to_vec();
    let train_indices: Vec<usize> = indices[val_size..].to_vec();

    let train_start = Instant::now();

    for epoch in 0..args.epochs {
        let mut train_idx = train_indices.clone();
        train_idx.shuffle(&mut rng);

        let mut train_loss = 0.0;
        let mut train_correct = 0usize;
        let n_batches = train_idx.len() / args.batch_size;

        for batch_i in 0..n_batches {
            let batch_indices: Vec<usize> = train_idx[batch_i * args.batch_size..(batch_i + 1) * args.batch_size].to_vec();
            let (features, targets, masks) = prepare_batch(&samples, &batch_indices);

            let logits = gat_policy.forward(&features, true);
            let masked_logits = logits + &masks;
            let log_probs = masked_logits.log_softmax(-1, Kind::Float);
            let loss = -log_probs
                .gather(1, &targets.unsqueeze(1), false)
                .squeeze_dim(1)
                .mean(Kind::Float);

            opt.backward_step(&loss);
            train_loss += f64::try_from(&loss).unwrap();

            let preds = masked_logits.argmax(-1, false);
            let correct: i64 = preds.eq_tensor(&targets).sum(Kind::Int64).int64_value(&[]);
            train_correct += correct as usize;
        }

        train_loss /= n_batches as f64;
        let train_acc = train_correct as f64 / (n_batches * args.batch_size) as f64;

        // Validation
        let (val_loss, val_acc) = evaluate(&gat_policy, &samples, &val_indices, args.batch_size);

        if epoch % 10 == 0 || epoch == args.epochs - 1 {
            println!("   Epoch {:3}/{:3} | Train: {:.4} ({:.1}%) | Val: {:.4} ({:.1}%)",
                     epoch + 1, args.epochs, train_loss, train_acc * 100.0, val_loss, val_acc * 100.0);
        }
    }

    println!("\n   Training completed in {:.1}s", train_start.elapsed().as_secs_f32());

    // ========== PHASE 3: Load CNN + Q-Net ==========
    println!("\n🔶 Loading CNN + Q-Net...");
    let neural_manager = NeuralManager::with_config(NeuralConfig::default()).ok();
    let qnet_manager = QNetManager::new("model_weights/qvalue_net.params").ok();

    if neural_manager.is_some() && qnet_manager.is_some() {
        println!("   ✅ Loaded CNN + Q-Net");
    } else {
        println!("   ⚠️ Could not load CNN/Q-Net (will skip comparison)");
    }

    // ========== PHASE 4: Evaluation ==========
    println!("\n📊 Evaluating on {} games with {} simulations...\n", args.games, args.simulations);

    let mut results: Vec<(&str, Vec<i32>, f64)> = Vec::new();

    // Random
    print!("   Random...");
    let start = Instant::now();
    let random_scores: Vec<i32> = (0..args.games)
        .map(|i| {
            let tiles = sample_tiles(&mut StdRng::seed_from_u64(args.seed + i as u64), 19);
            play_random(&tiles, &mut StdRng::seed_from_u64(args.seed + 10000 + i as u64))
        })
        .collect();
    println!(" {:.1}s, avg={:.1}", start.elapsed().as_secs_f32(), mean(&random_scores));
    results.push(("Random", random_scores, 0.0));

    // Greedy
    print!("   Greedy...");
    let start = Instant::now();
    let greedy_scores: Vec<i32> = (0..args.games)
        .map(|i| {
            let tiles = sample_tiles(&mut StdRng::seed_from_u64(args.seed + i as u64), 19);
            play_greedy(&tiles)
        })
        .collect();
    println!(" {:.1}s, avg={:.1}", start.elapsed().as_secs_f32(), mean(&greedy_scores));
    results.push(("Greedy", greedy_scores.clone(), 0.0));

    // GAT Supervised standalone
    print!("   GAT Supervised...");
    let start = Instant::now();
    let gat_scores: Vec<i32> = (0..args.games)
        .map(|i| {
            let tiles = sample_tiles(&mut StdRng::seed_from_u64(args.seed + i as u64), 19);
            play_gat_policy(&tiles, &gat_policy)
        })
        .collect();
    let elapsed = start.elapsed().as_secs_f32();
    println!(" {:.1}s, avg={:.1}", elapsed, mean(&gat_scores));
    results.push(("GAT Supervised", gat_scores.clone(), elapsed as f64));

    // GAT + MCTS
    print!("   GAT + MCTS...");
    let start = Instant::now();
    let gat_mcts_scores: Vec<i32> = (0..args.games)
        .map(|i| {
            let tiles = sample_tiles(&mut StdRng::seed_from_u64(args.seed + i as u64), 19);
            play_gat_mcts(&tiles, args.simulations, &gat_policy, args.top_k)
        })
        .collect();
    let elapsed = start.elapsed().as_secs_f32();
    println!(" {:.1}s, avg={:.1}", elapsed, mean(&gat_mcts_scores));
    results.push(("GAT + MCTS", gat_mcts_scores.clone(), elapsed as f64));

    // CNN + Q-Net MCTS
    if let (Some(ref nm), Some(ref qm)) = (&neural_manager, &qnet_manager) {
        print!("   CNN + Q-Net MCTS...");
        let start = Instant::now();
        let cnn_scores: Vec<i32> = (0..args.games)
            .map(|i| {
                let tiles = sample_tiles(&mut StdRng::seed_from_u64(args.seed + i as u64), 19);
                play_cnn_qnet(&tiles, args.simulations, nm, qm, args.top_k)
            })
            .collect();
        let elapsed = start.elapsed().as_secs_f32();
        println!(" {:.1}s, avg={:.1}", elapsed, mean(&cnn_scores));
        results.push(("CNN + Q-Net MCTS", cnn_scores, elapsed as f64));
    }

    // Pure MCTS
    print!("   Pure MCTS...");
    let start = Instant::now();
    let mcts_scores: Vec<i32> = (0..args.games)
        .map(|i| {
            let tiles = sample_tiles(&mut StdRng::seed_from_u64(args.seed + i as u64), 19);
            play_pure_mcts(&tiles, args.simulations)
        })
        .collect();
    let elapsed = start.elapsed().as_secs_f32();
    println!(" {:.1}s, avg={:.1}", elapsed, mean(&mcts_scores));
    results.push(("Pure MCTS", mcts_scores, elapsed as f64));

    // ========== PHASE 5: Results ==========
    let greedy_mean = mean(&greedy_scores);

    println!("\n╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                            RESULTS                                   ║");
    println!("╠══════════════════════════════════════════════════════════════════════╣");
    println!("║  Method                │   Avg   │   Min   │   Max   │ vs Greedy     ║");
    println!("╠══════════════════════════════════════════════════════════════════════╣");

    for (name, scores, _) in &results {
        let avg = mean(scores);
        let min = *scores.iter().min().unwrap_or(&0);
        let max = *scores.iter().max().unwrap_or(&0);
        println!("║  {:20} │ {:>7.2} │ {:>7} │ {:>7} │ {:>+7.2}       ║",
                 name, avg, min, max, avg - greedy_mean);
    }

    println!("╚══════════════════════════════════════════════════════════════════════╝");

    // Comparison
    let gat_avg = mean(&gat_scores);
    let gat_mcts_avg = mean(&gat_mcts_scores);
    let cnn_result = results.iter().find(|(n, _, _)| *n == "CNN + Q-Net MCTS");

    println!("\n📊 Analysis:");
    println!("   GAT Supervised: {:.2} pts", gat_avg);
    println!("   GAT + MCTS: {:.2} pts", gat_mcts_avg);

    if let Some((_, cnn_scores, _)) = cnn_result {
        let cnn_avg = mean(cnn_scores);
        println!("   CNN + Q-Net MCTS: {:.2} pts", cnn_avg);
        println!("\n   GAT Supervised vs CNN + Q-Net: {:+.2} pts", gat_avg - cnn_avg);
        println!("   GAT + MCTS vs CNN + Q-Net: {:+.2} pts", gat_mcts_avg - cnn_avg);

        if gat_mcts_avg > cnn_avg + 2.0 {
            println!("\n   🏆 GAT + MCTS BEATS CNN + Q-Net!");
        } else if gat_avg > cnn_avg + 2.0 {
            println!("\n   🏆 GAT Supervised alone BEATS CNN + Q-Net!");
        }
    }

    // GAT stats
    let above_100 = gat_scores.iter().filter(|&&s| s >= 100).count();
    let above_140 = gat_scores.iter().filter(|&&s| s >= 140).count();
    println!("\n📈 GAT Supervised stats:");
    println!("   Games >= 100 pts: {} ({:.1}%)", above_100, above_100 as f64 / args.games as f64 * 100.0);
    println!("   Games >= 140 pts: {} ({:.1}%)", above_140, above_140 as f64 / args.games as f64 * 100.0);
}

// ==================== Data Loading ====================

fn load_all_csv(dir: &str, min_score: i32) -> Vec<Sample> {
    let mut samples = Vec::new();
    let path = Path::new(dir);
    if !path.exists() { return samples; }

    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let file_path = entry.path();
        if file_path.extension().map_or(false, |e| e == "csv") {
            samples.extend(load_csv(&file_path, min_score));
        }
    }
    samples
}

fn load_csv(path: &Path, min_score: i32) -> Vec<Sample> {
    let mut samples = Vec::new();
    let file = match File::open(path) { Ok(f) => f, Err(_) => return samples };
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let _ = lines.next(); // skip header

    for line in lines {
        let line = match line { Ok(l) => l, Err(_) => continue };
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < 28 { continue; }

        let final_score: i32 = match fields[26].parse() { Ok(s) => s, Err(_) => continue };
        if final_score < min_score { continue; }

        let turn: usize = match fields[1].parse() { Ok(t) => t, Err(_) => continue };
        let mut plateau = [0i32; 19];
        for i in 0..19 { plateau[i] = fields[3 + i].parse().unwrap_or(0); }
        let tile = (
            fields[22].parse().unwrap_or(0),
            fields[23].parse().unwrap_or(0),
            fields[24].parse().unwrap_or(0),
        );
        let position: usize = match fields[25].parse() { Ok(p) => p, Err(_) => continue };

        samples.push(Sample { plateau, tile, position, turn });
    }
    samples
}

// ==================== Training Helpers ====================

fn decode_tile(encoded: i32) -> Tile {
    if encoded == 0 { return Tile(0, 0, 0); }
    Tile(encoded / 100, (encoded / 10) % 10, encoded % 10)
}

fn sample_to_features(sample: &Sample) -> Tensor {
    let mut plateau = Plateau { tiles: vec![Tile(0, 0, 0); 19] };
    for i in 0..19 { plateau.tiles[i] = decode_tile(sample.plateau[i]); }
    let tile = Tile(sample.tile.0, sample.tile.1, sample.tile.2);
    let deck = create_deck();
    convert_plateau_for_gat_47ch(&plateau, &tile, &deck, sample.turn, 19)
}

fn get_available_mask(sample: &Sample) -> Tensor {
    let mut mask = vec![f64::NEG_INFINITY; 19];
    for i in 0..19 { if sample.plateau[i] == 0 { mask[i] = 0.0; } }
    Tensor::from_slice(&mask)
}

fn prepare_batch(samples: &[Sample], indices: &[usize]) -> (Tensor, Tensor, Tensor) {
    let features: Vec<Tensor> = indices.iter().map(|&i| sample_to_features(&samples[i])).collect();
    let targets: Vec<i64> = indices.iter().map(|&i| samples[i].position as i64).collect();
    let masks: Vec<Tensor> = indices.iter().map(|&i| get_available_mask(&samples[i])).collect();
    (Tensor::stack(&features, 0), Tensor::from_slice(&targets), Tensor::stack(&masks, 0))
}

fn evaluate(net: &GATPolicyNet, samples: &[Sample], indices: &[usize], batch_size: usize) -> (f64, f64) {
    let n_batches = indices.len() / batch_size;
    if n_batches == 0 { return (0.0, 0.0); }

    let mut total_loss = 0.0;
    let mut total_correct = 0usize;

    for batch_i in 0..n_batches {
        let batch_indices: Vec<usize> = indices[batch_i * batch_size..(batch_i + 1) * batch_size].to_vec();
        let (features, targets, masks) = prepare_batch(samples, &batch_indices);

        let logits = net.forward(&features, false);
        let masked_logits = &logits + &masks;
        let log_probs = masked_logits.log_softmax(-1, Kind::Float);
        let loss = -log_probs.gather(1, &targets.unsqueeze(1), false).squeeze_dim(1).mean(Kind::Float);

        total_loss += f64::try_from(&loss).unwrap();
        let preds = masked_logits.argmax(-1, false);
        total_correct += preds.eq_tensor(&targets).sum(Kind::Int64).int64_value(&[]) as usize;
    }

    (total_loss / n_batches as f64, total_correct as f64 / (n_batches * batch_size) as f64)
}

// ==================== Game Playing ====================

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

fn play_random(tiles: &[Tile], rng: &mut StdRng) -> i32 {
    let mut plateau = create_plateau_empty();
    for tile in tiles {
        let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
        if avail.is_empty() { break; }
        plateau.tiles[*avail.choose(rng).unwrap()] = *tile;
    }
    result(&plateau)
}

fn play_greedy(tiles: &[Tile]) -> i32 {
    let mut plateau = create_plateau_empty();
    for tile in tiles {
        let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
        if avail.is_empty() { break; }
        let pos = avail.iter().copied().max_by_key(|&p| {
            let mut t = plateau.clone(); t.tiles[p] = *tile; result(&t)
        }).unwrap_or(avail[0]);
        plateau.tiles[pos] = *tile;
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
        for &pos in &avail { let _ = mask.i(pos as i64).fill_(0.0); }
        let best = (logits.squeeze_dim(0) + mask).argmax(0, false).int64_value(&[]) as usize;

        plateau.tiles[best] = *tile;
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

        let best_pos = if avail.len() > top_k + 2 {
            let features = convert_plateau_for_gat_47ch(&plateau, tile, &deck, turn, 19);
            let logits = policy.forward(&features.unsqueeze(0), false).squeeze_dim(0);

            let mut scored: Vec<(usize, f64)> = avail.iter()
                .map(|&pos| (pos, logits.double_value(&[pos as i64]))).collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let top_positions: Vec<usize> = scored.iter().take(top_k).map(|(p, _)| *p).collect();

            let sims_per_pos = num_sims / top_positions.len().max(1);
            let mut best = top_positions[0];
            let mut best_score = f64::NEG_INFINITY;

            for &pos in &top_positions {
                let mut temp = plateau.clone();
                temp.tiles[pos] = *tile;
                let temp_deck = replace_tile_in_deck(&deck, tile);
                let mut total = 0.0;
                for _ in 0..sims_per_pos {
                    // Use GAT policy for rollouts instead of random!
                    total += simulate_with_gat(&temp, &temp_deck, policy, turn + 1) as f64;
                }
                if total / sims_per_pos as f64 > best_score {
                    best_score = total / sims_per_pos as f64;
                    best = pos;
                }
            }
            best
        } else {
            mcts_find_best_position_for_tile_pure(&mut plateau.clone(), &mut deck.clone(), *tile, num_sims, turn, 19, None).best_position
        };

        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }
    result(&plateau)
}

fn play_cnn_qnet(tiles: &[Tile], num_sims: usize, nm: &NeuralManager, qm: &QNetManager, top_k: usize) -> i32 {
    use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_with_qnet;

    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let r = mcts_find_best_position_for_tile_with_qnet(
            &mut plateau, &mut deck, *tile, nm.policy_net(), nm.value_net(), qm.net(),
            num_sims, turn, 19, top_k, None,
        );
        plateau.tiles[r.best_position] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }
    result(&plateau)
}

fn play_pure_mcts(tiles: &[Tile], num_sims: usize) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let r = mcts_find_best_position_for_tile_pure(&mut plateau, &mut deck, *tile, num_sims, turn, 19, None);
        plateau.tiles[r.best_position] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }
    result(&plateau)
}

fn simulate_random(plateau: &Plateau, deck: &Deck) -> i32 {
    let mut rng = rand::rng();
    let mut p = plateau.clone();
    let mut d = deck.clone();

    loop {
        let tiles = get_available_tiles(&d);
        if tiles.is_empty() { break; }
        let tile = *tiles.choose(&mut rng).unwrap();
        let avail: Vec<usize> = (0..19).filter(|&i| p.tiles[i] == Tile(0, 0, 0)).collect();
        if avail.is_empty() { break; }
        p.tiles[*avail.choose(&mut rng).unwrap()] = tile;
        d = replace_tile_in_deck(&d, &tile);
    }
    result(&p)
}

/// Simulate game using GAT policy for rollouts (much better than random!)
fn simulate_with_gat(plateau: &Plateau, deck: &Deck, policy: &GATPolicyNet, start_turn: usize) -> i32 {
    let mut rng = rand::rng();
    let mut p = plateau.clone();
    let mut d = deck.clone();
    let mut turn = start_turn;

    loop {
        let tiles = get_available_tiles(&d);
        if tiles.is_empty() { break; }
        let tile = *tiles.choose(&mut rng).unwrap();

        let avail: Vec<usize> = (0..19).filter(|&i| p.tiles[i] == Tile(0, 0, 0)).collect();
        if avail.is_empty() { break; }

        // Use GAT policy to select position (not random!)
        let features = convert_plateau_for_gat_47ch(&p, &tile, &d, turn, 19);
        let logits = policy.forward(&features.unsqueeze(0), false).squeeze_dim(0);

        let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
        for &pos in &avail { let _ = mask.i(pos as i64).fill_(0.0); }
        let best = (logits + mask).argmax(0, false).int64_value(&[]) as usize;

        p.tiles[best] = tile;
        d = replace_tile_in_deck(&d, &tile);
        turn += 1;
    }
    result(&p)
}

fn mean(v: &[i32]) -> f64 {
    if v.is_empty() { 0.0 } else { v.iter().sum::<i32>() as f64 / v.len() as f64 }
}
