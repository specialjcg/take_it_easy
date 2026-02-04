//! Weighted Supervised Learning for GAT
//!
//! Trains on ALL data but weights samples by their final score.
//! Higher scores = higher weight in the loss function.
//!
//! Usage: cargo run --release --bin train_gat_weighted -- --min-score 100 --weight-power 2.0

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
use take_it_easy::game::plateau::Plateau;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::gat::GATPolicyNet;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;

#[derive(Parser, Debug)]
#[command(name = "train_gat_weighted")]
struct Args {
    /// Minimum score to include (use low value like 100)
    #[arg(long, default_value_t = 100)]
    min_score: i32,

    /// Weight power: weight = (score/100)^power
    #[arg(long, default_value_t = 2.0)]
    weight_power: f64,

    /// Training epochs
    #[arg(long, default_value_t = 80)]
    epochs: usize,

    /// Batch size
    #[arg(long, default_value_t = 64)]
    batch_size: usize,

    /// Learning rate
    #[arg(long, default_value_t = 0.001)]
    lr: f64,

    /// Hidden layer sizes
    #[arg(long, default_value = "128,128")]
    hidden: String,

    /// Number of attention heads
    #[arg(long, default_value_t = 4)]
    heads: usize,

    /// Dropout rate
    #[arg(long, default_value_t = 0.1)]
    dropout: f64,

    /// Weight decay (L2 regularization)
    #[arg(long, default_value_t = 0.0)]
    weight_decay: f64,

    /// Enable hexagonal rotation augmentation (6x data)
    #[arg(long, default_value_t = false)]
    augment: bool,

    /// LR scheduler: none, cosine, warmup_cosine
    #[arg(long, default_value = "none")]
    lr_scheduler: String,

    /// Warmup epochs (for warmup_cosine)
    #[arg(long, default_value_t = 5)]
    warmup_epochs: usize,

    /// Minimum LR ratio (for cosine: min_lr = lr * min_lr_ratio)
    #[arg(long, default_value_t = 0.01)]
    min_lr_ratio: f64,

    /// Validation split
    #[arg(long, default_value_t = 0.1)]
    val_split: f64,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Save model path
    #[arg(long, default_value = "model_weights/gat_weighted")]
    save_path: String,

    /// Data directory
    #[arg(long, default_value = "data")]
    data_dir: String,
}

#[derive(Clone)]
struct Sample {
    plateau: [i32; 19],
    tile: (i32, i32, i32),
    position: usize,
    turn: usize,
    final_score: i32,
    weight: f64,
}

/// 60° clockwise rotation mapping for the 19 hex positions
/// The board layout:
///       0   1   2
///     3   4   5   6
///   7   8   9  10  11
///    12  13  14  15
///      16  17  18
const ROTATE_60_CW: [usize; 19] = [
    2,   // 0 -> 2
    6,   // 1 -> 6
    11,  // 2 -> 11
    1,   // 3 -> 1
    5,   // 4 -> 5
    10,  // 5 -> 10
    15,  // 6 -> 15
    0,   // 7 -> 0
    4,   // 8 -> 4
    9,   // 9 -> 9 (center stays)
    14,  // 10 -> 14
    18,  // 11 -> 18
    3,   // 12 -> 3
    8,   // 13 -> 8
    13,  // 14 -> 13
    17,  // 15 -> 17
    7,   // 16 -> 7
    12,  // 17 -> 12
    16,  // 18 -> 16
];

/// Apply n times 60° rotation to a position
fn rotate_position(pos: usize, n: usize) -> usize {
    let mut p = pos;
    for _ in 0..(n % 6) {
        p = ROTATE_60_CW[p];
    }
    p
}

/// Rotate tile values: when board rotates 60° CW, line directions shift
/// Vertical(0) -> DiagRight(2), DiagLeft(1) -> Vertical(0), DiagRight(2) -> DiagLeft(1)
fn rotate_tile_values(tile: (i32, i32, i32), n: usize) -> (i32, i32, i32) {
    let mut t = tile;
    for _ in 0..(n % 6) {
        t = (t.1, t.2, t.0); // (v, dl, dr) -> (dl, dr, v)
    }
    t
}

/// Rotate encoded tile (v*100 + dl*10 + dr)
fn rotate_encoded_tile(encoded: i32, n: usize) -> i32 {
    if encoded == 0 { return 0; }
    let v = encoded / 100;
    let dl = (encoded / 10) % 10;
    let dr = encoded % 10;
    let (nv, ndl, ndr) = rotate_tile_values((v, dl, dr), n);
    nv * 100 + ndl * 10 + ndr
}

/// Apply rotation to a complete sample
fn rotate_sample(sample: &Sample, rotation: usize) -> Sample {
    if rotation == 0 { return sample.clone(); }

    let mut new_plateau = [0i32; 19];
    for i in 0..19 {
        let new_pos = rotate_position(i, rotation);
        new_plateau[new_pos] = rotate_encoded_tile(sample.plateau[i], rotation);
    }

    Sample {
        plateau: new_plateau,
        tile: rotate_tile_values(sample.tile, rotation),
        position: rotate_position(sample.position, rotation),
        turn: sample.turn,
        final_score: sample.final_score,
        weight: sample.weight,
    }
}

/// Compute learning rate with scheduling
fn compute_lr(
    base_lr: f64,
    epoch: usize,
    total_epochs: usize,
    scheduler: &str,
    warmup_epochs: usize,
    min_lr_ratio: f64,
) -> f64 {
    let min_lr = base_lr * min_lr_ratio;

    match scheduler {
        "cosine" => {
            // Cosine annealing: lr decreases from base_lr to min_lr following cosine curve
            let progress = epoch as f64 / total_epochs as f64;
            min_lr + 0.5 * (base_lr - min_lr) * (1.0 + (std::f64::consts::PI * progress).cos())
        }
        "warmup_cosine" => {
            if epoch < warmup_epochs {
                // Linear warmup
                base_lr * (epoch + 1) as f64 / warmup_epochs as f64
            } else {
                // Cosine annealing after warmup
                let remaining_epochs = total_epochs - warmup_epochs;
                let progress = (epoch - warmup_epochs) as f64 / remaining_epochs as f64;
                min_lr + 0.5 * (base_lr - min_lr) * (1.0 + (std::f64::consts::PI * progress).cos())
            }
        }
        _ => base_lr, // "none" or unknown
    }
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       GAT Weighted Supervised Learning                       ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let hidden: Vec<i64> = args.hidden.split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();

    println!("Config:");
    println!("  Min score:    {} pts", args.min_score);
    println!("  Weight power: {:.1} (weight = (score/100)^power)", args.weight_power);
    println!("  Epochs:       {}", args.epochs);
    println!("  Batch size:   {}", args.batch_size);
    println!("  LR:           {}", args.lr);
    println!("  Hidden:       {:?}", hidden);
    println!("  Heads:        {}", args.heads);
    println!("  Dropout:      {}", args.dropout);
    println!("  Weight decay: {}", args.weight_decay);
    println!("  LR scheduler: {}", args.lr_scheduler);
    if args.lr_scheduler == "warmup_cosine" {
        println!("  Warmup epochs:{}", args.warmup_epochs);
    }
    if args.lr_scheduler != "none" {
        println!("  Min LR ratio: {}", args.min_lr_ratio);
    }
    println!("  Augment:      {} (6x rotations)", args.augment);

    // Load data with weights
    println!("\n📂 Loading data from {}...", args.data_dir);
    let mut samples = load_all_csv_weighted(&args.data_dir, args.min_score, args.weight_power);
    let original_count = samples.len();
    println!("   Loaded {} samples (score >= {})", original_count, args.min_score);

    // Apply rotation augmentation if enabled
    if args.augment {
        println!("   Applying 6x rotation augmentation...");
        let mut augmented = Vec::with_capacity(samples.len() * 6);
        for sample in &samples {
            for rotation in 0..6 {
                augmented.push(rotate_sample(sample, rotation));
            }
        }
        samples = augmented;
        println!("   Augmented to {} samples (6x)", samples.len());
    }

    if samples.is_empty() {
        println!("❌ No samples found!");
        return;
    }

    // Show score/weight distribution
    let mut score_counts: HashMap<i32, usize> = HashMap::new();
    let mut total_weight = 0.0;
    for s in &samples {
        *score_counts.entry(s.final_score).or_insert(0) += 1;
        total_weight += s.weight;
    }
    let scores: Vec<_> = score_counts.keys().copied().collect();
    let min_s = *scores.iter().min().unwrap();
    let max_s = *scores.iter().max().unwrap();

    println!("   Score range: {} - {}", min_s, max_s);
    println!("   Weight examples:");
    println!("     Score 100 → weight {:.2}", (100.0_f64 / 100.0).powf(args.weight_power));
    println!("     Score 150 → weight {:.2}", (150.0_f64 / 100.0).powf(args.weight_power));
    println!("     Score 200 → weight {:.2}", (200.0_f64 / 100.0).powf(args.weight_power));
    println!("   Total weight: {:.1}, avg: {:.2}", total_weight, total_weight / samples.len() as f64);

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
    let vs = nn::VarStore::new(device);
    let policy_net = GATPolicyNet::new(&vs, 47, &hidden, args.heads, args.dropout);
    let mut opt = nn::Adam {
        wd: args.weight_decay,
        ..Default::default()
    }.build(&vs, args.lr).unwrap();

    let mut best_val_acc = 0.0f64;

    // Training loop
    println!("\n🏋️ Training with weighted loss...\n");
    for epoch in 0..args.epochs {
        let epoch_start = Instant::now();

        // Update learning rate based on scheduler
        let current_lr = compute_lr(
            args.lr,
            epoch,
            args.epochs,
            &args.lr_scheduler,
            args.warmup_epochs,
            args.min_lr_ratio,
        );
        opt.set_lr(current_lr);

        let mut train_idx = train_indices.clone();
        train_idx.shuffle(&mut rng);

        let mut train_loss = 0.0;
        let mut train_correct = 0usize;
        let mut _train_weight_sum = 0.0;
        let n_batches = train_idx.len() / args.batch_size;

        for batch_i in 0..n_batches {
            let batch_indices: Vec<usize> = train_idx[batch_i * args.batch_size..(batch_i + 1) * args.batch_size].to_vec();

            let (features, targets, masks, weights) = prepare_batch_weighted(&samples, &batch_indices);

            let logits = policy_net.forward(&features, true);
            let masked_logits = logits + &masks;
            let log_probs = masked_logits.log_softmax(-1, Kind::Float);

            // Per-sample loss
            let per_sample_loss = -log_probs
                .gather(1, &targets.unsqueeze(1), false)
                .squeeze_dim(1);

            // Weighted mean loss
            let weighted_loss = (&per_sample_loss * &weights).sum(Kind::Float) / weights.sum(Kind::Float);

            opt.backward_step(&weighted_loss);
            train_loss += f64::try_from(&weighted_loss).unwrap();
            _train_weight_sum += f64::try_from(&weights.sum(Kind::Float)).unwrap();

            // Accuracy (unweighted for comparison)
            let preds = masked_logits.argmax(-1, false);
            let correct: i64 = preds.eq_tensor(&targets).sum(Kind::Int64).int64_value(&[]);
            train_correct += correct as usize;
        }

        train_loss /= n_batches as f64;
        let train_acc = train_correct as f64 / (n_batches * args.batch_size) as f64;

        // Validation (unweighted)
        let (val_loss, val_acc) = evaluate(&policy_net, &samples, &val_indices, args.batch_size);

        let elapsed = epoch_start.elapsed().as_secs_f32();

        if epoch % 5 == 0 || epoch == args.epochs - 1 || val_acc > best_val_acc {
            let lr_info = if args.lr_scheduler != "none" {
                format!(" | LR: {:.6}", current_lr)
            } else {
                String::new()
            };
            println!("Epoch {:3}/{:3} | Train Loss: {:.4}, Acc: {:.2}% | Val Loss: {:.4}, Acc: {:.2}% | {:.1}s{}{}",
                     epoch + 1, args.epochs,
                     train_loss, train_acc * 100.0,
                     val_loss, val_acc * 100.0,
                     elapsed,
                     lr_info,
                     if val_acc > best_val_acc { " 💾" } else { "" });
        }

        if val_acc > best_val_acc {
            best_val_acc = val_acc;
            let _ = vs.save(format!("{}_policy.pt", args.save_path));
        }
    }

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                     TRAINING COMPLETE                        ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("\n  Best validation accuracy: {:.2}%", best_val_acc * 100.0);
    println!("  Model saved to: {}_policy.pt", args.save_path);

    // Evaluate
    println!("\n🎮 Evaluating by playing 200 games...\n");
    let (gat_avg, gat_scores) = eval_games(&policy_net, 200, &mut rng);
    let greedy_avg = eval_greedy(200, args.seed);

    println!("  GAT Weighted:   {:.2} pts", gat_avg);
    println!("  Greedy:         {:.2} pts", greedy_avg);
    println!("\n  vs Greedy: {:+.2} pts", gat_avg - greedy_avg);

    let above_100: usize = gat_scores.iter().filter(|&&s| s >= 100).count();
    let above_140: usize = gat_scores.iter().filter(|&&s| s >= 140).count();
    let above_150: usize = gat_scores.iter().filter(|&&s| s >= 150).count();
    println!("\n  Games >= 100 pts: {} ({:.1}%)", above_100, above_100 as f64 / 200.0 * 100.0);
    println!("  Games >= 140 pts: {} ({:.1}%)", above_140, above_140 as f64 / 200.0 * 100.0);
    println!("  Games >= 150 pts: {} ({:.1}%)", above_150, above_150 as f64 / 200.0 * 100.0);
}

fn load_all_csv_weighted(dir: &str, min_score: i32, weight_power: f64) -> Vec<Sample> {
    let mut samples = Vec::new();
    let path = Path::new(dir);
    if !path.exists() { return samples; }

    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let file_path = entry.path();
        if file_path.extension().map_or(false, |e| e == "csv") {
            let file_samples = load_csv_weighted(&file_path, min_score, weight_power);
            samples.extend(file_samples);
        }
    }
    samples
}

fn load_csv_weighted(path: &Path, min_score: i32, weight_power: f64) -> Vec<Sample> {
    let mut samples = Vec::new();
    let file = match File::open(path) { Ok(f) => f, Err(_) => return samples };
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let _ = lines.next(); // Skip header

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

        // Calculate weight: (score/100)^power
        let weight = (final_score as f64 / 100.0).powf(weight_power);

        samples.push(Sample { plateau, tile, position, turn, final_score, weight });
    }
    samples
}

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

fn prepare_batch_weighted(samples: &[Sample], indices: &[usize]) -> (Tensor, Tensor, Tensor, Tensor) {
    let features: Vec<Tensor> = indices.iter().map(|&i| sample_to_features(&samples[i])).collect();
    let targets: Vec<i64> = indices.iter().map(|&i| samples[i].position as i64).collect();
    let masks: Vec<Tensor> = indices.iter().map(|&i| get_available_mask(&samples[i])).collect();
    let weights: Vec<f64> = indices.iter().map(|&i| samples[i].weight).collect();

    (
        Tensor::stack(&features, 0),
        Tensor::from_slice(&targets),
        Tensor::stack(&masks, 0),
        Tensor::from_slice(&weights).to_kind(Kind::Float),
    )
}

fn evaluate(net: &GATPolicyNet, samples: &[Sample], indices: &[usize], batch_size: usize) -> (f64, f64) {
    let n_batches = indices.len() / batch_size;
    if n_batches == 0 { return (0.0, 0.0); }

    let mut total_loss = 0.0;
    let mut total_correct = 0usize;

    for batch_i in 0..n_batches {
        let batch_indices: Vec<usize> = indices[batch_i * batch_size..(batch_i + 1) * batch_size].to_vec();
        let (features, targets, masks, _) = prepare_batch_weighted(samples, &batch_indices);

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

fn eval_games(policy_net: &GATPolicyNet, n_games: usize, rng: &mut StdRng) -> (f64, Vec<i32>) {
    use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
    use take_it_easy::scoring::scoring::result;
    use take_it_easy::game::plateau::create_plateau_empty;

    let mut scores = Vec::new();
    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(rng).unwrap();

            let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if avail.is_empty() { break; }

            let features = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            let logits = policy_net.forward(&features.unsqueeze(0), false).squeeze_dim(0);

            let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
            for &pos in &avail { let _ = mask.i(pos as i64).fill_(0.0); }
            let best_pos = (logits + mask).argmax(0, false).int64_value(&[]) as usize;

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        scores.push(result(&plateau));
    }
    (scores.iter().sum::<i32>() as f64 / scores.len() as f64, scores)
}

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

            let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
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
