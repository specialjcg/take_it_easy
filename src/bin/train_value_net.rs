//! Value Network Training via GT Direct Self-Play
//!
//! Generates training data by playing games with GT Direct (gt_boosted_select),
//! then trains a value network V(state) -> expected final score.
//! Unlike train_graph_transformer_value.rs (which uses static CSV data),
//! this generates optimal-play data on the fly.
//!
//! Usage:
//!   cargo run --release --bin train_value_net -- --num-games 5000 --epochs 80
//!   cargo run --release --bin train_value_net -- --device cuda --num-games 10000 --eval-games 200

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;
use tch::{nn, nn::OptimizerConfig, Device, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::remove_tile_from_deck::replace_tile_in_deck;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::device_util::{check_cuda, parse_device};
use take_it_easy::neural::graph_transformer::{
    GraphTransformerPolicyNet, GraphTransformerValueNet,
};
use take_it_easy::neural::model_io::{load_varstore, save_varstore};
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;
use take_it_easy::strategy::expectimax::{expectimax_select, ExpectimaxConfig};

#[derive(Parser, Debug)]
#[command(name = "train_value_net")]
#[command(about = "Train value network from GT Direct self-play, then evaluate with expectimax")]
struct Args {
    /// Device: "cpu", "cuda", "cuda:0"
    #[arg(long, default_value = "cpu")]
    device: String,

    /// Number of games to generate training data
    #[arg(long, default_value_t = 5000)]
    num_games: usize,

    /// Training epochs
    #[arg(long, default_value_t = 80)]
    epochs: usize,

    /// Learning rate
    #[arg(long, default_value_t = 0.001)]
    lr: f64,

    /// Batch size
    #[arg(long, default_value_t = 128)]
    batch_size: usize,

    /// Number of evaluation games (expectimax vs GT Direct)
    #[arg(long, default_value_t = 100)]
    eval_games: usize,

    /// Line-boost strength for GT Direct
    #[arg(long, default_value_t = 3.0)]
    boost: f64,

    /// Path to save value network weights
    #[arg(long, default_value = "model_weights/value_net.safetensors")]
    model_path: String,

    /// Path to GT policy model weights
    #[arg(long, default_value = "model_weights/graph_transformer_policy.safetensors")]
    policy_path: String,

    /// Embedding dimension
    #[arg(long, default_value_t = 128)]
    embed_dim: i64,

    /// Number of transformer layers
    #[arg(long, default_value_t = 2)]
    num_layers: usize,

    /// Number of attention heads
    #[arg(long, default_value_t = 4)]
    num_heads: i64,

    /// Dropout rate
    #[arg(long, default_value_t = 0.1)]
    dropout: f64,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Score normalization mean
    #[arg(long, default_value_t = 140.0)]
    score_mean: f64,

    /// Score normalization std
    #[arg(long, default_value_t = 40.0)]
    score_std: f64,

    /// Validation split ratio
    #[arg(long, default_value_t = 0.1)]
    val_split: f64,

    /// Minimum LR ratio for cosine schedule
    #[arg(long, default_value_t = 0.01)]
    min_lr_ratio: f64,

    /// Weight decay
    #[arg(long, default_value_t = 0.0001)]
    weight_decay: f64,

    /// Data directory with CSV files (skip self-play, load from CSVs instead)
    #[arg(long)]
    data_dir: Option<String>,

    /// Minimum score to include from CSV data
    #[arg(long, default_value_t = 0)]
    min_score: i32,
}

struct Sample {
    features: Tensor, // [19, 47]
    final_score: i32,
}

fn main() {
    let args = Args::parse();

    println!("================================================");
    println!("  Value Network Training (GT Direct Self-Play)");
    println!("================================================\n");

    let device = match parse_device(&args.device) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    };
    check_cuda();
    println!("Device: {:?}", device);
    println!("Training: {} epochs, lr={}, batch_size={}", args.epochs, args.lr, args.batch_size);
    println!("Architecture: embed={}, layers={}, heads={}", args.embed_dim, args.num_layers, args.num_heads);

    // Load policy network on the target device
    let mut policy_vs = nn::VarStore::new(device);
    let policy_net = GraphTransformerPolicyNet::new(
        &policy_vs, 47, args.embed_dim, args.num_layers, args.num_heads, 0.0,
    );

    if !Path::new(&args.policy_path).exists() {
        eprintln!("Error: policy model not found: {}", args.policy_path);
        return;
    }
    match load_varstore(&mut policy_vs, &args.policy_path) {
        Ok(()) => println!("Loaded policy model from {}", args.policy_path),
        Err(e) => {
            eprintln!("Error loading policy model: {}", e);
            return;
        }
    }

    // ── Phase 1: Load or generate training data ──
    println!("\n--- Phase 1: Data ---\n");
    let gen_start = Instant::now();

    let samples = if let Some(ref data_dir) = args.data_dir {
        println!("Loading CSV data from {}...", data_dir);
        let csv_samples = load_all_csv(data_dir, args.min_score);
        println!("Loaded {} CSV rows (score >= {})", csv_samples.len(), args.min_score);
        if csv_samples.is_empty() {
            eprintln!("No CSV data found!");
            return;
        }
        // Convert CSV samples to tensor features
        csv_samples
            .iter()
            .map(|s| {
                let mut plateau = Plateau { tiles: vec![Tile(0, 0, 0); 19] };
                for i in 0..19 {
                    plateau.tiles[i] = decode_tile(s.plateau[i]);
                }
                let tile = Tile(s.tile.0, s.tile.1, s.tile.2);
                let deck = create_deck();
                Sample {
                    features: convert_plateau_for_gat_47ch(&plateau, &tile, &deck, s.turn, 19),
                    final_score: s.final_score,
                }
            })
            .collect::<Vec<_>>()
    } else {
        println!("Generating {} games with GT Direct (boost={:.1})...", args.num_games, args.boost);
        generate_data(&policy_net, device, &args)
    };

    let gen_time = gen_start.elapsed().as_secs_f32();
    println!("{} samples ready ({:.1}s)", samples.len(), gen_time);

    // Score statistics
    let scores: Vec<f64> = samples.iter().map(|s| s.final_score as f64).collect();
    let mean = scores.iter().sum::<f64>() / scores.len() as f64;
    let std = (scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / scores.len() as f64).sqrt();
    println!("Score stats: mean={:.1}, std={:.1}", mean, std);

    if samples.is_empty() {
        eprintln!("No samples generated!");
        return;
    }

    // ── Phase 2: Train value network ──
    println!("\n--- Phase 2: Training ---\n");
    let value_vs = nn::VarStore::new(device);
    let value_net = GraphTransformerValueNet::new(
        &value_vs, 47, args.embed_dim, args.num_layers, args.num_heads, args.dropout,
    );

    let mut opt = nn::Adam {
        wd: args.weight_decay,
        ..Default::default()
    }
    .build(&value_vs, args.lr)
    .unwrap();

    // Split train/val
    let mut rng = StdRng::seed_from_u64(args.seed + 999);
    let mut indices: Vec<usize> = (0..samples.len()).collect();
    indices.shuffle(&mut rng);

    let val_size = (samples.len() as f64 * args.val_split) as usize;
    let val_indices: Vec<usize> = indices[..val_size].to_vec();
    let train_indices: Vec<usize> = indices[val_size..].to_vec();
    println!(
        "Train: {} samples, Val: {} samples",
        train_indices.len(),
        val_indices.len()
    );

    let train_start = Instant::now();
    let mut best_val_loss = f64::INFINITY;

    for epoch in 0..args.epochs {
        let epoch_start = Instant::now();

        // Cosine LR schedule
        let lr = compute_lr(args.lr, epoch, args.epochs, args.min_lr_ratio);
        opt.set_lr(lr);

        // Training
        let mut train_loss_sum = 0.0;
        let mut train_mae_sum = 0.0;
        let mut train_count = 0usize;

        let mut train_perm = train_indices.clone();
        train_perm.shuffle(&mut rng);

        for batch_start in (0..train_perm.len()).step_by(args.batch_size) {
            let batch_end = (batch_start + args.batch_size).min(train_perm.len());
            let batch_idx = &train_perm[batch_start..batch_end];

            let (features, targets) = prepare_batch(&samples, batch_idx, device, args.score_mean, args.score_std);

            let predictions = value_net.forward(&features, true);
            let loss = predictions.mse_loss(&targets, tch::Reduction::Mean);
            opt.backward_step(&loss);

            let n = batch_idx.len();
            train_loss_sum += loss.double_value(&[]) * n as f64;

            let pred_pts = predictions.squeeze() * args.score_std + args.score_mean;
            let true_pts = targets.squeeze() * args.score_std + args.score_mean;
            let mae: f64 = (pred_pts - true_pts).abs().mean(Kind::Float).double_value(&[]);
            train_mae_sum += mae * n as f64;
            train_count += n;
        }

        let train_loss = train_loss_sum / train_count as f64;
        let train_mae = train_mae_sum / train_count as f64;

        // Validation
        let mut val_loss_sum = 0.0;
        let mut val_mae_sum = 0.0;
        let mut val_count = 0usize;

        for batch_start in (0..val_indices.len()).step_by(args.batch_size) {
            let batch_end = (batch_start + args.batch_size).min(val_indices.len());
            let batch_idx = &val_indices[batch_start..batch_end];

            let (features, targets) = prepare_batch(&samples, batch_idx, device, args.score_mean, args.score_std);

            let predictions = tch::no_grad(|| value_net.forward(&features, false));
            let loss = predictions.mse_loss(&targets, tch::Reduction::Mean);

            let n = batch_idx.len();
            val_loss_sum += loss.double_value(&[]) * n as f64;

            let pred_pts = predictions.squeeze() * args.score_std + args.score_mean;
            let true_pts = targets.squeeze() * args.score_std + args.score_mean;
            let mae: f64 = (pred_pts - true_pts).abs().mean(Kind::Float).double_value(&[]);
            val_mae_sum += mae * n as f64;
            val_count += n;
        }

        let val_loss = val_loss_sum / val_count as f64;
        let val_mae = val_mae_sum / val_count as f64;
        let epoch_time = epoch_start.elapsed().as_secs_f32();

        let saved = if val_loss < best_val_loss {
            best_val_loss = val_loss;
            if let Err(e) = save_varstore(&value_vs, &args.model_path) {
                eprintln!("Warning: failed to save: {}", e);
            }
            true
        } else {
            false
        };

        if epoch % 5 == 0 || saved || epoch == args.epochs - 1 {
            print!(
                "Epoch {:3}/{:3} | Train L:{:.4} MAE:{:.1} | Val L:{:.4} MAE:{:.1} | {:.1}s | lr:{:.6}",
                epoch + 1, args.epochs, train_loss, train_mae, val_loss, val_mae, epoch_time, lr
            );
            if saved {
                print!(" *");
            }
            println!();
        }
    }

    let train_time = train_start.elapsed().as_secs_f32();
    println!("\nTraining complete in {:.1}s", train_time);
    println!("Best val loss: {:.4} (MAE ~{:.1} pts)", best_val_loss, best_val_loss.sqrt() * args.score_std);
    println!("Model saved to: {}", args.model_path);

    // ── Phase 3: Evaluate with Expectimax ──
    if args.eval_games > 0 {
        println!("\n--- Phase 3: Evaluation ({} games) ---\n", args.eval_games);

        // Reload best value net weights
        let mut eval_value_vs = nn::VarStore::new(device);
        let eval_value_net = GraphTransformerValueNet::new(
            &eval_value_vs, 47, args.embed_dim, args.num_layers, args.num_heads, 0.0,
        );
        if let Err(e) = load_varstore(&mut eval_value_vs, &args.model_path) {
            eprintln!("Error reloading value net: {}", e);
            return;
        }

        let expectimax_config = ExpectimaxConfig {
            device,
            boost: args.boost,
            score_mean: args.score_mean,
            score_std: args.score_std,
        };

        let mut eval_rng = StdRng::seed_from_u64(args.seed + 2000);
        let mut gt_scores = Vec::with_capacity(args.eval_games);
        let mut ex_scores = Vec::with_capacity(args.eval_games);

        for i in 0..args.eval_games {
            let tile_seq = random_tile_sequence(&mut eval_rng);

            // GT Direct
            let gt = play_gt_direct(&policy_net, device, &tile_seq, args.boost);
            gt_scores.push(gt);

            // Expectimax
            let ex = play_expectimax(
                &policy_net,
                &eval_value_net,
                &expectimax_config,
                &tile_seq,
            );
            ex_scores.push(ex);

            if (i + 1) % 20 == 0 {
                let gt_avg = gt_scores.iter().sum::<i32>() as f64 / gt_scores.len() as f64;
                let ex_avg = ex_scores.iter().sum::<i32>() as f64 / ex_scores.len() as f64;
                println!(
                    "  [{:>4}/{}] GT Direct: {:.1}  Expectimax: {:.1}  (delta: {:+.1})",
                    i + 1,
                    args.eval_games,
                    gt_avg,
                    ex_avg,
                    ex_avg - gt_avg
                );
            }
        }

        let gt_avg = gt_scores.iter().sum::<i32>() as f64 / gt_scores.len() as f64;
        let ex_avg = ex_scores.iter().sum::<i32>() as f64 / ex_scores.len() as f64;
        let gt_std = std_dev(&gt_scores);
        let ex_std = std_dev(&ex_scores);

        println!("\n{}", "=".repeat(60));
        println!(
            "{:<16} {:>8} {:>8} {:>8} {:>8}",
            "Strategy", "Avg", "Std", "Min", "Max"
        );
        println!("{}", "-".repeat(60));
        println!(
            "{:<16} {:>8.1} {:>8.1} {:>8} {:>8}",
            "GT Direct",
            gt_avg,
            gt_std,
            gt_scores.iter().min().unwrap(),
            gt_scores.iter().max().unwrap()
        );
        println!(
            "{:<16} {:>8.1} {:>8.1} {:>8} {:>8}  ({:+.1})",
            "Expectimax",
            ex_avg,
            ex_std,
            ex_scores.iter().min().unwrap(),
            ex_scores.iter().max().unwrap(),
            ex_avg - gt_avg
        );
        println!("{}", "=".repeat(60));
    }
}

/// Generate training data by playing games with GT Direct (GPU-accelerated).
///
/// Policy inference runs on the target device (GPU if available).
/// Features are created on CPU then moved to device for the forward pass.
fn generate_data(
    policy_net: &GraphTransformerPolicyNet,
    device: Device,
    args: &Args,
) -> Vec<Sample> {
    use take_it_easy::strategy::gt_boost::line_boost;

    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut samples = Vec::with_capacity(args.num_games * 19);

    for game_idx in 0..args.num_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut game_features: Vec<Tensor> = Vec::with_capacity(19);

        let tile_seq = random_tile_sequence(&mut rng);

        for (turn, &tile) in tile_seq.iter().enumerate() {
            deck = replace_tile_in_deck(&deck, &tile);

            // Record features on CPU BEFORE making the move
            let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            game_features.push(feat.shallow_clone());

            let legal = get_legal_moves(&plateau);
            if legal.is_empty() {
                break;
            }

            // GT Direct inference on device (GPU)
            let feat_device = feat.unsqueeze(0).to_device(device);
            let logits = tch::no_grad(|| policy_net.forward(&feat_device, false))
                .squeeze_dim(0)
                .to_device(Device::Cpu);
            let logit_values: Vec<f64> = Vec::<f64>::try_from(&logits).unwrap();

            // Mask + line_boost → argmax
            let pos = *legal
                .iter()
                .max_by(|&&a, &&b| {
                    let sa = logit_values[a] + line_boost(&plateau, &tile, a, args.boost);
                    let sb = logit_values[b] + line_boost(&plateau, &tile, b, args.boost);
                    sa.partial_cmp(&sb).unwrap()
                })
                .unwrap();

            plateau.tiles[pos] = tile;
        }

        let final_score = result(&plateau);

        for feat in game_features {
            samples.push(Sample {
                features: feat,
                final_score,
            });
        }

        if (game_idx + 1) % 500 == 0 {
            println!(
                "  Generated {}/{} games (last score: {})",
                game_idx + 1,
                args.num_games,
                final_score
            );
        }
    }

    samples
}

fn prepare_batch(
    samples: &[Sample],
    indices: &[usize],
    device: Device,
    score_mean: f64,
    score_std: f64,
) -> (Tensor, Tensor) {
    let features: Vec<Tensor> = indices.iter().map(|&i| samples[i].features.shallow_clone()).collect();
    let targets: Vec<f64> = indices
        .iter()
        .map(|&i| (samples[i].final_score as f64 - score_mean) / score_std)
        .collect();

    let features_batch = Tensor::stack(&features, 0).to_device(device);
    let targets_batch = Tensor::from_slice(&targets)
        .to_kind(Kind::Float)
        .unsqueeze(1)
        .to_device(device);

    (features_batch, targets_batch)
}

fn compute_lr(base_lr: f64, epoch: usize, total_epochs: usize, min_lr_ratio: f64) -> f64 {
    let min_lr = base_lr * min_lr_ratio;
    let progress = epoch as f64 / total_epochs as f64;
    min_lr + 0.5 * (base_lr - min_lr) * (1.0 + (std::f64::consts::PI * progress).cos())
}

fn random_tile_sequence(rng: &mut StdRng) -> Vec<Tile> {
    let deck = create_deck();
    let mut available: Vec<Tile> = deck
        .tiles()
        .iter()
        .copied()
        .filter(|t| *t != Tile(0, 0, 0))
        .collect();
    let mut seq = Vec::with_capacity(19);
    for _ in 0..19 {
        if available.is_empty() {
            break;
        }
        let idx = rng.random_range(0..available.len());
        seq.push(available.remove(idx));
    }
    seq
}

/// Play one game with GT Direct + line_boost (GPU-aware).
fn play_gt_direct(
    policy_net: &GraphTransformerPolicyNet,
    device: Device,
    tile_sequence: &[Tile],
    boost: f64,
) -> i32 {
    use take_it_easy::strategy::gt_boost::line_boost;

    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, &tile) in tile_sequence.iter().enumerate() {
        deck = replace_tile_in_deck(&deck, &tile);
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19)
            .unsqueeze(0)
            .to_device(device);
        let logits = tch::no_grad(|| policy_net.forward(&feat, false))
            .squeeze_dim(0)
            .to_device(Device::Cpu);
        let logit_values: Vec<f64> = Vec::<f64>::try_from(&logits).unwrap();

        let pos = *legal
            .iter()
            .max_by(|&&a, &&b| {
                let sa = logit_values[a] + line_boost(&plateau, &tile, a, boost);
                let sb = logit_values[b] + line_boost(&plateau, &tile, b, boost);
                sa.partial_cmp(&sb).unwrap()
            })
            .unwrap();

        plateau.tiles[pos] = tile;
    }

    result(&plateau)
}

/// Play one game with expectimax strategy.
fn play_expectimax(
    policy_net: &GraphTransformerPolicyNet,
    value_net: &GraphTransformerValueNet,
    config: &ExpectimaxConfig,
    tile_sequence: &[Tile],
) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, &tile) in tile_sequence.iter().enumerate() {
        deck = replace_tile_in_deck(&deck, &tile);
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }
        let pos = expectimax_select(&plateau, &tile, &deck, turn, policy_net, value_net, config);
        plateau.tiles[pos] = tile;
    }

    result(&plateau)
}

fn std_dev(scores: &[i32]) -> f64 {
    let mean = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
    let var = scores.iter().map(|&s| (s as f64 - mean).powi(2)).sum::<f64>() / scores.len() as f64;
    var.sqrt()
}

// ── CSV loading (reused from train_graph_transformer_value.rs) ──

#[derive(Clone)]
struct CsvSample {
    plateau: [i32; 19],
    tile: (i32, i32, i32),
    turn: usize,
    final_score: i32,
}

fn load_all_csv(dir: &str, min_score: i32) -> Vec<CsvSample> {
    let mut samples = Vec::new();
    let path = Path::new(dir);
    if !path.exists() {
        return samples;
    }
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let file_path = entry.path();
        if file_path.extension().map_or(false, |e| e == "csv") {
            samples.extend(load_csv(&file_path, min_score));
        }
    }
    samples
}

fn load_csv(path: &Path, min_score: i32) -> Vec<CsvSample> {
    let mut samples = Vec::new();
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return samples,
    };
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let _ = lines.next(); // skip header

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
        samples.push(CsvSample {
            plateau,
            tile,
            turn,
            final_score,
        });
    }
    samples
}

fn decode_tile(encoded: i32) -> Tile {
    if encoded == 0 {
        return Tile(0, 0, 0);
    }
    Tile(encoded / 100, (encoded / 10) % 10, encoded % 10)
}
