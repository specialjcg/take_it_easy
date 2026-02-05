//! GAT with Jumping Knowledge Training
//!
//! Trains GAT with Jumping Knowledge aggregation - combines representations
//! from all layers instead of just the final layer.
//!
//! Usage: cargo run --release --bin train_gat_jk -- --jk-mode concat

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
use take_it_easy::neural::gat_jk::{GATJKPolicyNet, JKMode};
use take_it_easy::neural::model_io::save_varstore;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;

#[derive(Parser, Debug)]
#[command(name = "train_gat_jk")]
struct Args {
    /// JK aggregation mode: concat, max, attention
    #[arg(long, default_value = "concat")]
    jk_mode: String,

    /// Minimum score to include
    #[arg(long, default_value_t = 100)]
    min_score: i32,

    /// Weight power: weight = (score/100)^power
    #[arg(long, default_value_t = 3.0)]
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
    #[arg(long, default_value_t = 0.2)]
    dropout: f64,

    /// Weight decay (L2 regularization)
    #[arg(long, default_value_t = 0.0001)]
    weight_decay: f64,

    /// LR scheduler: none, cosine
    #[arg(long, default_value = "cosine")]
    lr_scheduler: String,

    /// Minimum LR ratio (for cosine)
    #[arg(long, default_value_t = 0.01)]
    min_lr_ratio: f64,

    /// Validation split
    #[arg(long, default_value_t = 0.1)]
    val_split: f64,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Save model path
    #[arg(long, default_value = "model_weights/gat_jk")]
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

fn compute_lr(base_lr: f64, epoch: usize, total_epochs: usize, scheduler: &str, min_lr_ratio: f64) -> f64 {
    let min_lr = base_lr * min_lr_ratio;
    match scheduler {
        "cosine" => {
            let progress = epoch as f64 / total_epochs as f64;
            min_lr + 0.5 * (base_lr - min_lr) * (1.0 + (std::f64::consts::PI * progress).cos())
        }
        _ => base_lr,
    }
}

fn main() {
    let args = Args::parse();
    let jk_mode = JKMode::from_str(&args.jk_mode);

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       GAT + Jumping Knowledge Training                       ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let hidden: Vec<i64> = args.hidden.split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();

    println!("Config:");
    println!("  Architecture: GAT + Jumping Knowledge");
    println!("  JK Mode:      {:?}", jk_mode);
    println!("  Min score:    {} pts", args.min_score);
    println!("  Weight power: {:.1}", args.weight_power);
    println!("  Epochs:       {}", args.epochs);
    println!("  Hidden:       {:?}", hidden);
    println!("  Heads:        {}", args.heads);
    println!("  Dropout:      {}", args.dropout);
    println!("  LR scheduler: {}", args.lr_scheduler);

    // Load data
    println!("\n Loading data from {}...", args.data_dir);
    let samples = load_all_csv_weighted(&args.data_dir, args.min_score, args.weight_power);
    println!("   Loaded {} samples (score >= {})", samples.len(), args.min_score);

    if samples.is_empty() {
        println!("No samples found!");
        return;
    }

    // Score distribution
    let mut score_counts: HashMap<i32, usize> = HashMap::new();
    let mut total_weight = 0.0;
    for s in &samples {
        *score_counts.entry(s.final_score).or_insert(0) += 1;
        total_weight += s.weight;
    }
    let scores: Vec<_> = score_counts.keys().copied().collect();
    println!("   Score range: {} - {}", scores.iter().min().unwrap(), scores.iter().max().unwrap());
    println!("   Total weight: {:.1}", total_weight);

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
    let policy_net = GATJKPolicyNet::new(&vs, 47, &hidden, args.heads, args.dropout, jk_mode);
    let mut opt = nn::Adam {
        wd: args.weight_decay,
        ..Default::default()
    }.build(&vs, args.lr).unwrap();

    let mut best_val_acc = 0.0f64;
    let mut best_game_score = 0.0f64;

    // Training loop
    println!("\n Training GAT-JK ({:?})...\n", jk_mode);
    for epoch in 0..args.epochs {
        let epoch_start = Instant::now();

        let current_lr = compute_lr(args.lr, epoch, args.epochs, &args.lr_scheduler, args.min_lr_ratio);
        opt.set_lr(current_lr);

        let mut train_idx = train_indices.clone();
        train_idx.shuffle(&mut rng);

        let mut train_loss = 0.0;
        let mut train_correct = 0usize;
        let n_batches = train_idx.len() / args.batch_size;

        for batch_i in 0..n_batches {
            let batch_indices: Vec<usize> = train_idx[batch_i * args.batch_size..(batch_i + 1) * args.batch_size].to_vec();

            let (features, targets, masks, weights) = prepare_batch_weighted(&samples, &batch_indices);

            let logits = policy_net.forward(&features, true);
            let masked_logits = logits + &masks;
            let log_probs = masked_logits.log_softmax(-1, Kind::Float);

            let per_sample_loss = -log_probs.gather(1, &targets.unsqueeze(1), false).squeeze_dim(1);
            let weighted_loss = (&per_sample_loss * &weights).sum(Kind::Float) / weights.sum(Kind::Float);

            opt.backward_step(&weighted_loss);
            train_loss += f64::try_from(&weighted_loss).unwrap();

            let preds = masked_logits.argmax(-1, false);
            let correct: i64 = preds.eq_tensor(&targets).sum(Kind::Int64).int64_value(&[]);
            train_correct += correct as usize;
        }

        train_loss /= n_batches as f64;
        let train_acc = train_correct as f64 / (n_batches * args.batch_size) as f64;

        // Validation
        let (val_loss, val_acc) = evaluate(&policy_net, &samples, &val_indices, args.batch_size);

        let elapsed = epoch_start.elapsed().as_secs_f32();

        // Game evaluation every 10 epochs
        let should_eval = epoch % 10 == 9 || epoch == args.epochs - 1;
        let game_score = if should_eval {
            let (score, _) = eval_games(&policy_net, 100, &mut rng);
            score
        } else {
            0.0
        };

        let improved = val_acc > best_val_acc || (should_eval && game_score > best_game_score);

        if epoch % 5 == 0 || epoch == args.epochs - 1 || improved {
            let lr_info = format!(" | LR: {:.6}", current_lr);
            let game_info = if should_eval { format!(" | Game: {:.1} pts", game_score) } else { String::new() };
            println!("Epoch {:3}/{:3} | Train Loss: {:.4}, Acc: {:.2}% | Val Loss: {:.4}, Acc: {:.2}% | {:.1}s{}{}{}",
                     epoch + 1, args.epochs,
                     train_loss, train_acc * 100.0,
                     val_loss, val_acc * 100.0,
                     elapsed, lr_info, game_info,
                     if improved { " *" } else { "" });
        }

        if val_acc > best_val_acc {
            best_val_acc = val_acc;
        }

        if should_eval && game_score > best_game_score {
            best_game_score = game_score;
            let path = format!("{}_{}_policy.safetensors", args.save_path, args.jk_mode);
            if let Err(e) = save_varstore(&vs, &path) {
                eprintln!("Warning: failed to save: {}", e);
            }
            println!("   New best game score! Model saved.");
        }
    }

    println!("\n══════════════════════════════════════════════════════════════");
    println!("                     TRAINING COMPLETE");
    println!("══════════════════════════════════════════════════════════════");
    println!("\n  JK Mode: {:?}", jk_mode);
    println!("  Best validation accuracy: {:.2}%", best_val_acc * 100.0);
    println!("  Best game score: {:.2} pts", best_game_score);

    // Final evaluation
    println!("\n Evaluating by playing 200 games...\n");
    let (gat_avg, gat_scores) = eval_games(&policy_net, 200, &mut rng);
    let greedy_avg = eval_greedy(200, args.seed);

    println!("  GAT-JK ({:?}): {:.2} pts", jk_mode, gat_avg);
    println!("  Greedy:           {:.2} pts", greedy_avg);
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
            samples.extend(load_csv_weighted(&file_path, min_score, weight_power));
        }
    }
    samples
}

fn load_csv_weighted(path: &Path, min_score: i32, weight_power: f64) -> Vec<Sample> {
    let mut samples = Vec::new();
    let file = match File::open(path) { Ok(f) => f, Err(_) => return samples };
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let _ = lines.next();

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

fn evaluate(net: &GATJKPolicyNet, samples: &[Sample], indices: &[usize], batch_size: usize) -> (f64, f64) {
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

fn eval_games(policy_net: &GATJKPolicyNet, n_games: usize, rng: &mut StdRng) -> (f64, Vec<i32>) {
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
