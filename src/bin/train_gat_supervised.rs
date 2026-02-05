//! Supervised Learning for GAT from high-score games
//!
//! Trains GAT policy network using behavioral cloning on games with score >= threshold.
//! Much faster than self-play since no game generation is needed.
//!
//! Usage: cargo run --release --bin train_gat_supervised -- --min-score 140

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;
use tch::{nn, nn::OptimizerConfig, Device, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::Plateau;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::gat::GATPolicyNet;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;

#[derive(Parser, Debug)]
#[command(name = "train_gat_supervised")]
struct Args {
    /// Minimum final score to include game
    #[arg(long, default_value_t = 140)]
    min_score: i32,

    /// Training epochs
    #[arg(long, default_value_t = 100)]
    epochs: usize,

    /// Batch size
    #[arg(long, default_value_t = 64)]
    batch_size: usize,

    /// Learning rate
    #[arg(long, default_value_t = 0.001)]
    lr: f64,

    /// Hidden layer sizes (comma-separated)
    #[arg(long, default_value = "128,128")]
    hidden: String,

    /// Number of attention heads
    #[arg(long, default_value_t = 4)]
    heads: usize,

    /// Validation split
    #[arg(long, default_value_t = 0.1)]
    val_split: f64,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Save model path
    #[arg(long, default_value = "model_weights/gat_supervised")]
    save_path: String,

    /// Data directory
    #[arg(long, default_value = "data")]
    data_dir: String,
}

/// Training sample from CSV
#[derive(Clone)]
struct Sample {
    plateau: [i32; 19],  // Encoded tiles (0 = empty)
    tile: (i32, i32, i32),
    position: usize,
    turn: usize,
    final_score: i32,
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       GAT Supervised Learning from High-Score Games         ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let hidden: Vec<i64> = args.hidden.split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();

    println!("Config:");
    println!("  Min score:  {} pts", args.min_score);
    println!("  Epochs:     {}", args.epochs);
    println!("  Batch size: {}", args.batch_size);
    println!("  LR:         {}", args.lr);
    println!("  Hidden:     {:?}", hidden);
    println!("  Heads:      {}", args.heads);

    // Load data
    println!("\n📂 Loading data from {}...", args.data_dir);
    let samples = load_all_csv(&args.data_dir, args.min_score);
    println!("   Loaded {} samples from games with score >= {}", samples.len(), args.min_score);

    if samples.is_empty() {
        println!("❌ No samples found!");
        return;
    }

    // Show score distribution
    let mut score_counts: HashMap<i32, usize> = HashMap::new();
    for s in &samples {
        *score_counts.entry(s.final_score).or_insert(0) += 1;
    }
    let mut scores: Vec<_> = score_counts.keys().collect();
    scores.sort();
    println!("   Score range: {} - {}", scores.first().unwrap(), scores.last().unwrap());

    // Split train/val
    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut indices: Vec<usize> = (0..samples.len()).collect();
    indices.shuffle(&mut rng);

    let val_size = (samples.len() as f64 * args.val_split) as usize;
    let val_indices: Vec<usize> = indices[..val_size].to_vec();
    let train_indices: Vec<usize> = indices[val_size..].to_vec();

    println!("   Train: {} samples, Val: {} samples", train_indices.len(), val_indices.len());

    // Initialize network
    let device = Device::Cpu;
    let mut vs = nn::VarStore::new(device);
    let policy_net = GATPolicyNet::new(&vs, 47, &hidden, args.heads, 0.1);
    let mut opt = nn::Adam::default().build(&vs, args.lr).unwrap();

    let mut best_val_acc = 0.0f64;

    // Training loop
    println!("\n🏋️ Training...\n");
    for epoch in 0..args.epochs {
        let epoch_start = Instant::now();

        // Shuffle training indices
        let mut train_idx = train_indices.clone();
        train_idx.shuffle(&mut rng);

        // Training
        let mut train_loss = 0.0;
        let mut train_correct = 0usize;
        let n_batches = train_idx.len() / args.batch_size;

        for batch_i in 0..n_batches {
            let batch_indices: Vec<usize> = train_idx[batch_i * args.batch_size..(batch_i + 1) * args.batch_size].to_vec();

            let (features, targets, masks) = prepare_batch(&samples, &batch_indices);

            let logits = policy_net.forward(&features, true);

            // Masked cross-entropy loss
            let masked_logits = logits + &masks;
            let log_probs = masked_logits.log_softmax(-1, Kind::Float);
            let loss = -log_probs
                .gather(1, &targets.unsqueeze(1), false)
                .squeeze_dim(1)
                .mean(Kind::Float);

            opt.backward_step(&loss);
            train_loss += f64::try_from(&loss).unwrap();

            // Accuracy
            let preds = masked_logits.argmax(-1, false);
            let correct: i64 = preds.eq_tensor(&targets).sum(Kind::Int64).int64_value(&[]);
            train_correct += correct as usize;
        }

        train_loss /= n_batches as f64;
        let train_acc = train_correct as f64 / (n_batches * args.batch_size) as f64;

        // Validation
        let (val_loss, val_acc) = evaluate(&policy_net, &samples, &val_indices, args.batch_size);

        let elapsed = epoch_start.elapsed().as_secs_f32();

        // Print progress
        if epoch % 5 == 0 || epoch == args.epochs - 1 || val_acc > best_val_acc {
            println!("Epoch {:3}/{:3} | Train Loss: {:.4}, Acc: {:.2}% | Val Loss: {:.4}, Acc: {:.2}% | {:.1}s{}",
                     epoch + 1, args.epochs,
                     train_loss, train_acc * 100.0,
                     val_loss, val_acc * 100.0,
                     elapsed,
                     if val_acc > best_val_acc { " 💾" } else { "" });
        }

        // Save best model
        if val_acc > best_val_acc {
            best_val_acc = val_acc;
            let _ = vs.save(format!("{}_policy.pt", args.save_path));
        }
    }

    // Final results
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                     TRAINING COMPLETE                        ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("\n  Best validation accuracy: {:.2}%", best_val_acc * 100.0);
    println!("  Model saved to: {}_policy.pt", args.save_path);

    // Evaluate by playing games
    println!("\n🎮 Evaluating by playing 200 games...\n");
    let (gat_avg, gat_scores) = eval_games(&policy_net, 200, &mut rng);
    let greedy_avg = eval_greedy(200, args.seed);
    let random_avg = eval_random(200, args.seed);

    println!("  GAT Supervised: {:.2} pts", gat_avg);
    println!("  Greedy:         {:.2} pts", greedy_avg);
    println!("  Random:         {:.2} pts", random_avg);
    println!();
    println!("  vs Greedy: {:+.2} pts", gat_avg - greedy_avg);
    println!("  vs Random: {:+.2} pts", gat_avg - random_avg);

    let above_100: usize = gat_scores.iter().filter(|&&s| s >= 100).count();
    let above_140: usize = gat_scores.iter().filter(|&&s| s >= 140).count();
    println!("\n  Games >= 100 pts: {} ({:.1}%)", above_100, above_100 as f64 / 200.0 * 100.0);
    println!("  Games >= 140 pts: {} ({:.1}%)", above_140, above_140 as f64 / 200.0 * 100.0);

    if gat_avg > greedy_avg {
        println!("\n  🏆 GAT BEATS GREEDY!");
    }
}

/// Evaluate by playing games
fn eval_games(policy_net: &GATPolicyNet, n_games: usize, rng: &mut StdRng) -> (f64, Vec<i32>) {
    use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
    use take_it_easy::scoring::scoring::result;
    use take_it_easy::game::plateau::create_plateau_empty;
    use tch::IndexOp;

    let mut scores = Vec::new();

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(rng).unwrap();

            let avail: Vec<usize> = (0..19)
                .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
                .collect();
            if avail.is_empty() { break; }

            let features = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            let logits = policy_net.forward(&features.unsqueeze(0), false).squeeze_dim(0);

            let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
            for &pos in &avail {
                let _ = mask.i(pos as i64).fill_(0.0);
            }
            let masked_logits = logits + mask;
            let best_pos = masked_logits.argmax(0, false).int64_value(&[]) as usize;

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        scores.push(result(&plateau));
    }

    let avg = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
    (avg, scores)
}

/// Evaluate greedy baseline
fn eval_greedy(n_games: usize, seed: u64) -> f64 {
    use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
    use take_it_easy::scoring::scoring::result;
    use take_it_easy::game::plateau::create_plateau_empty;

    let mut rng = StdRng::seed_from_u64(seed + 10000);
    let mut total = 0;

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for _ in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(&mut rng).unwrap();

            let avail: Vec<usize> = (0..19)
                .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
                .collect();
            if avail.is_empty() { break; }

            let best_pos = avail.iter().copied().max_by_key(|&pos| {
                let mut test = plateau.clone();
                test.tiles[pos] = tile;
                result(&test)
            }).unwrap_or(avail[0]);

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        total += result(&plateau);
    }

    total as f64 / n_games as f64
}

/// Evaluate random baseline
fn eval_random(n_games: usize, seed: u64) -> f64 {
    use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
    use take_it_easy::scoring::scoring::result;
    use take_it_easy::game::plateau::create_plateau_empty;

    let mut rng = StdRng::seed_from_u64(seed + 20000);
    let mut total = 0;

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for _ in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(&mut rng).unwrap();

            let avail: Vec<usize> = (0..19)
                .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
                .collect();
            if avail.is_empty() { break; }

            let pos = *avail.choose(&mut rng).unwrap();
            plateau.tiles[pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        total += result(&plateau);
    }

    total as f64 / n_games as f64
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
            final_score,
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
    // Reconstruct plateau
    let mut plateau = Plateau {
        tiles: vec![Tile(0, 0, 0); 19],
    };
    for i in 0..19 {
        plateau.tiles[i] = decode_tile(sample.plateau[i]);
    }

    // Current tile
    let tile = Tile(sample.tile.0, sample.tile.1, sample.tile.2);

    // Create deck and remove played tiles (approximation)
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
    let batch_size = indices.len();

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

/// Evaluate on validation set
fn evaluate(
    net: &GATPolicyNet,
    samples: &[Sample],
    indices: &[usize],
    batch_size: usize,
) -> (f64, f64) {
    let n_batches = indices.len() / batch_size;
    if n_batches == 0 {
        return (0.0, 0.0);
    }

    let mut total_loss = 0.0;
    let mut total_correct = 0usize;

    for batch_i in 0..n_batches {
        let batch_indices: Vec<usize> = indices[batch_i * batch_size..(batch_i + 1) * batch_size].to_vec();

        let (features, targets, masks) = prepare_batch(samples, &batch_indices);

        let logits = net.forward(&features, false);

        let masked_logits = &logits + &masks;
        let log_probs = masked_logits.log_softmax(-1, Kind::Float);
        let loss = -log_probs
            .gather(1, &targets.unsqueeze(1), false)
            .squeeze_dim(1)
            .mean(Kind::Float);

        total_loss += f64::try_from(&loss).unwrap();

        let preds = masked_logits.argmax(-1, false);
        let correct: i64 = preds.eq_tensor(&targets).sum(Kind::Int64).int64_value(&[]);
        total_correct += correct as usize;
    }

    (
        total_loss / n_batches as f64,
        total_correct as f64 / (n_batches * batch_size) as f64,
    )
}
