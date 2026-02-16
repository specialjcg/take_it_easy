//! Graph Transformer Value Network Training
//!
//! Trains a value network to predict final game scores from any position.
//! Uses the same supervised data as policy training.
//!
//! Usage: cargo run --release --bin train_graph_transformer_value -- --epochs 80

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;
use tch::{nn, nn::OptimizerConfig, Device, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::Plateau;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::graph_transformer::GraphTransformerValueNet;
use take_it_easy::neural::model_io::save_varstore;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "train_graph_transformer_value")]
#[command(about = "Train Graph Transformer Value Network to predict final scores")]
struct Args {
    /// Minimum score to include
    #[arg(long, default_value_t = 100)]
    min_score: i32,

    /// Training epochs
    #[arg(long, default_value_t = 80)]
    epochs: usize,

    /// Batch size
    #[arg(long, default_value_t = 64)]
    batch_size: usize,

    /// Learning rate
    #[arg(long, default_value_t = 0.001)]
    lr: f64,

    /// Dropout rate
    #[arg(long, default_value_t = 0.1)]
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
    #[arg(long, default_value = "model_weights/graph_transformer_value.safetensors")]
    save_path: String,

    /// Data directory
    #[arg(long, default_value = "data")]
    data_dir: String,

    /// Score normalization mean
    #[arg(long, default_value_t = 140.0)]
    score_mean: f64,

    /// Score normalization std
    #[arg(long, default_value_t = 40.0)]
    score_std: f64,
}

#[derive(Clone)]
struct Sample {
    plateau: [i32; 19],
    tile: (i32, i32, i32),
    turn: usize,
    final_score: i32,
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║    Graph Transformer Value Network Training                  ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("Config:");
    println!("  Min score:    {} pts", args.min_score);
    println!("  Epochs:       {}", args.epochs);
    println!("  Batch size:   {}", args.batch_size);
    println!("  LR:           {}", args.lr);
    println!("  Architecture: Graph Transformer (47 input, 128 embed, 2 layers, 4 heads)");
    println!("  Dropout:      {}", args.dropout);
    println!("  Weight decay: {}", args.weight_decay);
    println!("  LR scheduler: {}", args.lr_scheduler);
    println!("  Score norm:   mean={}, std={}", args.score_mean, args.score_std);

    // Load data
    println!("\n📂 Loading data from {}...", args.data_dir);
    let samples = load_all_csv(&args.data_dir, args.min_score);
    println!("   Loaded {} samples (score >= {})", samples.len(), args.min_score);

    if samples.is_empty() {
        println!("❌ No samples found!");
        return;
    }

    // Score statistics
    let scores: Vec<f64> = samples.iter().map(|s| s.final_score as f64).collect();
    let mean_score = scores.iter().sum::<f64>() / scores.len() as f64;
    let std_score = (scores.iter().map(|s| (s - mean_score).powi(2)).sum::<f64>() / scores.len() as f64).sqrt();
    let min_score = scores.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_score = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    println!("   Score range: {:.0} - {:.0}", min_score, max_score);
    println!("   Score mean: {:.2}, std: {:.2}", mean_score, std_score);

    // Split train/val
    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut indices: Vec<usize> = (0..samples.len()).collect();
    indices.shuffle(&mut rng);

    let val_size = (samples.len() as f64 * args.val_split) as usize;
    let val_indices: Vec<usize> = indices[..val_size].to_vec();
    let train_indices: Vec<usize> = indices[val_size..].to_vec();

    println!("   Train: {} samples, Val: {} samples", train_indices.len(), val_indices.len());

    // Create network
    let device = Device::Cpu;
    let vs = nn::VarStore::new(device);
    let net = GraphTransformerValueNet::new(&vs, 47, 128, 2, 4, args.dropout);

    let mut opt = nn::Adam {
        wd: args.weight_decay,
        ..Default::default()
    }.build(&vs, args.lr).unwrap();

    println!("\n🏋️ Training value network...\n");

    let mut best_val_loss = f64::INFINITY;
    let start = Instant::now();

    for epoch in 0..args.epochs {
        let epoch_start = Instant::now();

        // Update learning rate
        let lr = compute_lr(args.lr, epoch, args.epochs, &args.lr_scheduler, args.min_lr_ratio);
        opt.set_lr(lr);

        // Training
        let mut train_loss = 0.0;
        let mut train_mae = 0.0;
        let mut train_count = 0;

        let mut train_perm = train_indices.clone();
        train_perm.shuffle(&mut rng);

        for batch_start in (0..train_perm.len()).step_by(args.batch_size) {
            let batch_end = (batch_start + args.batch_size).min(train_perm.len());
            let batch_indices = &train_perm[batch_start..batch_end];

            let (features, targets) = prepare_batch(
                &samples, batch_indices, device, args.score_mean, args.score_std
            );

            let predictions = net.forward(&features, true);
            let loss = predictions.mse_loss(&targets, tch::Reduction::Mean);

            opt.backward_step(&loss);

            let loss_val: f64 = loss.double_value(&[]);
            train_loss += loss_val * batch_indices.len() as f64;

            // MAE in original scale
            let pred_scores = predictions.squeeze() * args.score_std + args.score_mean;
            let true_scores = targets.squeeze() * args.score_std + args.score_mean;
            let mae: f64 = (pred_scores - true_scores).abs().mean(Kind::Float).double_value(&[]);
            train_mae += mae * batch_indices.len() as f64;

            train_count += batch_indices.len();
        }

        train_loss /= train_count as f64;
        train_mae /= train_count as f64;

        // Validation
        let mut val_loss = 0.0;
        let mut val_mae = 0.0;
        let mut val_count = 0;

        for batch_start in (0..val_indices.len()).step_by(args.batch_size) {
            let batch_end = (batch_start + args.batch_size).min(val_indices.len());
            let batch_indices = &val_indices[batch_start..batch_end];

            let (features, targets) = prepare_batch(
                &samples, batch_indices, device, args.score_mean, args.score_std
            );

            let predictions = tch::no_grad(|| net.forward(&features, false));
            let loss = predictions.mse_loss(&targets, tch::Reduction::Mean);

            let loss_val: f64 = loss.double_value(&[]);
            val_loss += loss_val * batch_indices.len() as f64;

            let pred_scores = predictions.squeeze() * args.score_std + args.score_mean;
            let true_scores = targets.squeeze() * args.score_std + args.score_mean;
            let mae: f64 = (pred_scores - true_scores).abs().mean(Kind::Float).double_value(&[]);
            val_mae += mae * batch_indices.len() as f64;

            val_count += batch_indices.len();
        }

        val_loss /= val_count as f64;
        val_mae /= val_count as f64;

        let epoch_time = epoch_start.elapsed().as_secs_f32();

        // Save best model
        let saved = if val_loss < best_val_loss {
            best_val_loss = val_loss;
            if let Err(e) = save_varstore(&vs, &args.save_path) {
                eprintln!("Warning: failed to save model: {}", e);
            }
            true
        } else {
            false
        };

        // Print progress (every 5 epochs or if saved)
        if epoch % 5 == 0 || saved || epoch == args.epochs - 1 {
            print!("Epoch {:3}/{:3} | Train Loss: {:.4}, MAE: {:.1} | Val Loss: {:.4}, MAE: {:.1} | {:.1}s | LR: {:.6}",
                epoch + 1, args.epochs, train_loss, train_mae, val_loss, val_mae, epoch_time, lr);
            if saved { print!(" 💾"); }
            println!();
        }
    }

    let total_time = start.elapsed().as_secs_f32();

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                     TRAINING COMPLETE                        ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("  Best validation loss: {:.4}", best_val_loss);
    println!("  Best validation MAE:  ~{:.1} pts", best_val_loss.sqrt() * args.score_std);
    println!("  Total time: {:.1}s", total_time);
    println!("  Model saved to: {}", args.save_path);

    // Evaluate on games
    println!("\n🎮 Evaluating value predictions on 100 games...");
    evaluate_value_network(&net, &args, 100);
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

fn prepare_batch(
    samples: &[Sample],
    indices: &[usize],
    device: Device,
    score_mean: f64,
    score_std: f64,
) -> (Tensor, Tensor) {
    let features: Vec<Tensor> = indices.iter()
        .map(|&i| sample_to_features(&samples[i]))
        .collect();

    let targets: Vec<f64> = indices.iter()
        .map(|&i| (samples[i].final_score as f64 - score_mean) / score_std)
        .collect();

    let features_batch = Tensor::stack(&features, 0).to_device(device);
    let targets_batch = Tensor::from_slice(&targets)
        .to_kind(Kind::Float)
        .unsqueeze(1)
        .to_device(device);

    (features_batch, targets_batch)
}

fn load_all_csv(dir: &str, min_score: i32) -> Vec<Sample> {
    let mut samples = Vec::new();
    let path = Path::new(dir);
    if !path.exists() { return samples; }

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

fn load_csv(path: &Path, min_score: i32) -> Vec<Sample> {
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

        samples.push(Sample { plateau, tile, turn, final_score });
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

fn evaluate_value_network(net: &GraphTransformerValueNet, args: &Args, n_games: usize) {
    let mut rng = StdRng::seed_from_u64(args.seed + 1000);

    let mut prediction_errors = Vec::new();
    let mut early_predictions = Vec::new();
    let mut mid_predictions = Vec::new();
    let mut late_predictions = Vec::new();

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut game_predictions: Vec<(usize, f64)> = Vec::new();

        for turn in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(&mut rng).unwrap();

            // Get value prediction at this state
            let features = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            let pred_normalized = tch::no_grad(|| {
                net.forward(&features.unsqueeze(0), false).double_value(&[0, 0])
            });
            let pred_score = pred_normalized * args.score_std + args.score_mean;
            game_predictions.push((turn, pred_score));

            // Make random move
            let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if avail.is_empty() { break; }
            let pos = *avail.choose(&mut rng).unwrap();
            plateau.tiles[pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        let final_score = result(&plateau) as f64;

        for (turn, pred) in &game_predictions {
            let error = (pred - final_score).abs();
            prediction_errors.push(error);

            if *turn < 6 {
                early_predictions.push(error);
            } else if *turn < 13 {
                mid_predictions.push(error);
            } else {
                late_predictions.push(error);
            }
        }
    }

    let avg_error = prediction_errors.iter().sum::<f64>() / prediction_errors.len() as f64;
    let early_error = early_predictions.iter().sum::<f64>() / early_predictions.len().max(1) as f64;
    let mid_error = mid_predictions.iter().sum::<f64>() / mid_predictions.len().max(1) as f64;
    let late_error = late_predictions.iter().sum::<f64>() / late_predictions.len().max(1) as f64;

    println!("\n  Value Prediction Accuracy:");
    println!("    Overall MAE:     {:.1} pts", avg_error);
    println!("    Early game (0-5):  {:.1} pts", early_error);
    println!("    Mid game (6-12):   {:.1} pts", mid_error);
    println!("    Late game (13-18): {:.1} pts", late_error);
}
