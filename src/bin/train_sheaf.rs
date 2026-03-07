//! Sheaf Neural Network Training
//!
//! Trains a Sheaf Neural Network that uses direction-dependent restriction maps
//! and the sheaf Laplacian to propagate information along scoring lines.
//! Same training pipeline as train_graph_transformer but with the novel sheaf architecture.
//!
//! Usage:
//!   cargo run --release --bin train_sheaf -- --device cuda --gen-games 100000 \
//!     --policy-path model_weights/graph_transformer_policy.safetensors

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use tch::{nn, nn::OptimizerConfig, Device, IndexOp, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::remove_tile_from_deck::replace_tile_in_deck;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::device_util::{check_cuda, parse_device};
use take_it_easy::neural::graph_transformer::GraphTransformerPolicyNet;
use take_it_easy::neural::model_io::{load_varstore, save_varstore};
use take_it_easy::neural::sheaf_network::SheafPolicyNet;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;
use take_it_easy::strategy::gt_boost::line_boost;

#[derive(Parser, Debug)]
#[command(name = "train_sheaf")]
struct Args {
    #[arg(long, default_value = "cpu")]
    device: String,

    #[arg(long, default_value_t = 100)]
    min_score: i32,

    #[arg(long, default_value_t = 3.0)]
    weight_power: f64,

    #[arg(long, default_value_t = 80)]
    epochs: usize,

    #[arg(long, default_value_t = 64)]
    batch_size: usize,

    #[arg(long, default_value_t = 0.0005)]
    lr: f64,

    #[arg(long, default_value_t = 128)]
    embed_dim: i64,

    #[arg(long, default_value_t = 64)]
    stalk_dim: i64,

    #[arg(long, default_value_t = 3)]
    num_layers: usize,

    #[arg(long, default_value_t = 0.1)]
    dropout: f64,

    #[arg(long, default_value_t = 0.0001)]
    weight_decay: f64,

    #[arg(long, default_value = "cosine")]
    lr_scheduler: String,

    #[arg(long, default_value_t = 0.01)]
    min_lr_ratio: f64,

    #[arg(long, default_value_t = 0.1)]
    val_split: f64,

    #[arg(long, default_value_t = 3)]
    patience: usize,

    #[arg(long, default_value_t = 42)]
    seed: u64,

    #[arg(long, default_value = "model_weights/sheaf")]
    save_path: String,

    /// Number of GT Direct self-play games to generate (0 = use CSV)
    #[arg(long, default_value_t = 0)]
    gen_games: usize,

    /// Policy model for self-play generation (GT Direct)
    #[arg(long, default_value = "model_weights/graph_transformer_policy.safetensors")]
    policy_path: String,

    /// Generator embed dim (must match policy-path model)
    #[arg(long, default_value_t = 128)]
    gen_embed_dim: i64,

    #[arg(long, default_value_t = 2)]
    gen_num_layers: usize,

    #[arg(long, default_value_t = 4)]
    gen_heads: i64,

    #[arg(long, default_value_t = 3.0)]
    boost: f64,

    /// CSV data directory (when gen-games == 0)
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

fn main() {
    let args = Args::parse();

    let device = match parse_device(&args.device) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    };
    check_cuda();

    println!("================================================================");
    println!("         Sheaf Neural Network Training");
    println!("================================================================\n");

    println!("Config:");
    println!("  Device:       {:?}", device);
    println!("  Architecture: Sheaf Neural Network (direction-aware Laplacian)");
    println!("  Embed dim:    {}", args.embed_dim);
    println!("  Stalk dim:    {} (restriction map bottleneck)", args.stalk_dim);
    println!("  Layers:       {}", args.num_layers);
    println!("  FF dim:       {} (4x embed)", args.embed_dim * 4);
    println!("  Dropout:      {}", args.dropout);
    println!("  Weight decay: {}", args.weight_decay);
    println!("  Epochs:       {}", args.epochs);
    println!("  LR:           {} ({})", args.lr, args.lr_scheduler);
    println!("  Weight power: {:.1}", args.weight_power);
    if args.gen_games > 0 {
        println!(
            "  Data:         {} self-play games (GT Direct)",
            args.gen_games
        );
        println!(
            "  Gen arch:     dim={}, layers={}, heads={}",
            args.gen_embed_dim, args.gen_num_layers, args.gen_heads
        );
    } else {
        println!("  Data:         CSV from {}", args.data_dir);
        println!("  Min score:    {} pts", args.min_score);
    }

    // Load or generate data
    let samples = if args.gen_games > 0 {
        generate_selfplay_data(&args, device)
    } else {
        println!("\n Loading data from {}...", args.data_dir);
        let s = load_all_csv_weighted(&args.data_dir, args.min_score, args.weight_power);
        println!("   Loaded {} samples (score >= {})", s.len(), args.min_score);
        s
    };

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
    println!(
        "   Score range: {} - {}",
        scores.iter().min().unwrap(),
        scores.iter().max().unwrap()
    );
    println!("   Total weight: {:.1}", total_weight);

    // Pre-compute all features on device
    println!("\n Pre-computing features on {:?}...", device);
    let precompute_start = Instant::now();
    let n = samples.len();
    let all_features: Vec<Tensor> = samples.iter().map(|s| sample_to_features(s)).collect();
    let all_masks: Vec<Tensor> = samples.iter().map(|s| get_available_mask(s)).collect();
    let all_targets: Vec<i64> = samples.iter().map(|s| s.position as i64).collect();
    let all_weights: Vec<f64> = samples.iter().map(|s| s.weight).collect();

    let features_gpu = Tensor::stack(&all_features, 0).to_device(device);
    let masks_gpu = Tensor::stack(&all_masks, 0).to_device(device);
    let targets_gpu = Tensor::from_slice(&all_targets).to_device(device);
    let weights_gpu = Tensor::from_slice(&all_weights)
        .to_kind(Kind::Float)
        .to_device(device);
    drop(all_features);
    drop(all_masks);
    println!(
        "   Pre-computed {} samples in {:.1}s",
        n,
        precompute_start.elapsed().as_secs_f32()
    );

    // Split train/val
    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut indices: Vec<usize> = (0..n).collect();
    indices.shuffle(&mut rng);

    let val_size = (n as f64 * args.val_split) as usize;
    let val_indices: Vec<usize> = indices[..val_size].to_vec();
    let train_indices: Vec<usize> = indices[val_size..].to_vec();
    println!(
        "   Train: {} samples, Val: {} samples",
        train_indices.len(),
        val_indices.len()
    );

    // Initialize Sheaf Neural Network
    let vs = nn::VarStore::new(device);
    let policy_net = SheafPolicyNet::new(
        &vs,
        47,
        args.embed_dim,
        args.stalk_dim,
        args.num_layers,
        args.dropout,
    );

    let param_count: i64 = vs.variables().values().map(|t| t.numel() as i64).sum();
    println!(
        "   Parameters: {} ({:.1}K)",
        param_count,
        param_count as f64 / 1e3
    );

    let mut opt = nn::Adam {
        wd: args.weight_decay,
        ..Default::default()
    }
    .build(&vs, args.lr)
    .unwrap();

    let mut best_val_acc = 0.0f64;
    let mut best_game_score = 0.0f64;
    let mut evals_without_improvement = 0usize;

    println!("\n Training Sheaf Neural Network...\n");
    for epoch in 0..args.epochs {
        let epoch_start = Instant::now();

        let current_lr = compute_lr(
            args.lr,
            epoch,
            args.epochs,
            &args.lr_scheduler,
            args.min_lr_ratio,
        );
        opt.set_lr(current_lr);

        let mut train_idx = train_indices.clone();
        train_idx.shuffle(&mut rng);

        let mut train_loss = 0.0;
        let mut train_correct = 0usize;
        let n_batches = train_idx.len() / args.batch_size;

        for batch_i in 0..n_batches {
            let start = batch_i * args.batch_size;
            let end = start + args.batch_size;
            let batch_idx: Vec<i64> = train_idx[start..end].iter().map(|&i| i as i64).collect();
            let idx_tensor = Tensor::from_slice(&batch_idx).to_device(device);

            let features = features_gpu.index_select(0, &idx_tensor);
            let targets = targets_gpu.index_select(0, &idx_tensor);
            let masks = masks_gpu.index_select(0, &idx_tensor);
            let weights = weights_gpu.index_select(0, &idx_tensor);

            let logits = policy_net.forward(&features, true);
            let masked_logits = logits + &masks;
            let log_probs = masked_logits.log_softmax(-1, Kind::Float);

            let per_sample_loss =
                -log_probs
                    .gather(1, &targets.unsqueeze(1), false)
                    .squeeze_dim(1);
            let weighted_loss =
                (&per_sample_loss * &weights).sum(Kind::Float) / weights.sum(Kind::Float);

            opt.backward_step(&weighted_loss);
            train_loss += f64::try_from(&weighted_loss).unwrap();

            let preds = masked_logits.argmax(-1, false);
            let correct: i64 = preds.eq_tensor(&targets).sum(Kind::Int64).int64_value(&[]);
            train_correct += correct as usize;
        }

        train_loss /= n_batches as f64;
        let train_acc = train_correct as f64 / (n_batches * args.batch_size) as f64;

        // Validation
        let (val_loss, val_acc) = evaluate_gpu(
            &policy_net,
            &features_gpu,
            &targets_gpu,
            &masks_gpu,
            &val_indices,
            args.batch_size,
            device,
        );

        let elapsed = epoch_start.elapsed().as_secs_f32();

        // Game evaluation every 10 epochs
        let should_eval = epoch % 10 == 9 || epoch == args.epochs - 1;
        let game_score = if should_eval {
            let (score, _) = eval_games(&policy_net, 100, &mut rng, device);
            score
        } else {
            0.0
        };

        let improved = val_acc > best_val_acc || (should_eval && game_score > best_game_score);

        if epoch % 5 == 0 || epoch == args.epochs - 1 || improved {
            let game_info = if should_eval {
                format!(" | Game: {:.1} pts", game_score)
            } else {
                String::new()
            };
            println!(
                "Epoch {:3}/{:3} | Train L:{:.4} Acc:{:.1}% | Val L:{:.4} Acc:{:.1}% | {:.1}s{}{}",
                epoch + 1,
                args.epochs,
                train_loss,
                train_acc * 100.0,
                val_loss,
                val_acc * 100.0,
                elapsed,
                game_info,
                if improved { " *" } else { "" }
            );
        }

        if val_acc > best_val_acc {
            best_val_acc = val_acc;
        }

        if should_eval && game_score > best_game_score {
            best_game_score = game_score;
            evals_without_improvement = 0;
            let path = format!("{}_policy.safetensors", args.save_path);
            if let Err(e) = save_varstore(&vs, &path) {
                eprintln!("Warning: failed to save: {}", e);
            }
            println!("   New best game score! Model saved.");
        } else if should_eval {
            evals_without_improvement += 1;
            if args.patience > 0 && evals_without_improvement >= args.patience {
                println!(
                    "\n   Early stopping: {} evals without improvement (best: {:.1} pts)",
                    args.patience, best_game_score
                );
                break;
            }
        }
    }

    println!("\n================================================================");
    println!("                     TRAINING COMPLETE");
    println!("================================================================");
    println!("\n  Architecture: Sheaf Neural Network");
    println!("  Embed dim: {}, Stalk dim: {}", args.embed_dim, args.stalk_dim);
    println!("  Best validation accuracy: {:.2}%", best_val_acc * 100.0);
    println!("  Best game score: {:.2} pts", best_game_score);

    // Final evaluation
    println!("\n Evaluating by playing 200 games...\n");
    let (sheaf_avg, sheaf_scores) = eval_games(&policy_net, 200, &mut rng, device);

    println!("  Sheaf Network:       {:.2} pts", sheaf_avg);
    println!("  Reference GT Direct: ~152.3 pts (baseline)");
    println!("\n  vs GT Direct: {:+.2} pts", sheaf_avg - 152.3);

    let above_100: usize = sheaf_scores.iter().filter(|&&s| s >= 100).count();
    let above_140: usize = sheaf_scores.iter().filter(|&&s| s >= 140).count();
    let above_150: usize = sheaf_scores.iter().filter(|&&s| s >= 150).count();
    println!(
        "\n  Games >= 100 pts: {} ({:.1}%)",
        above_100,
        above_100 as f64 / 200.0 * 100.0
    );
    println!(
        "  Games >= 140 pts: {} ({:.1}%)",
        above_140,
        above_140 as f64 / 200.0 * 100.0
    );
    println!(
        "  Games >= 150 pts: {} ({:.1}%)",
        above_150,
        above_150 as f64 / 200.0 * 100.0
    );
}

// -- Self-play data generation (uses existing GT Direct policy) ----------

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

fn generate_selfplay_data(args: &Args, device: Device) -> Vec<Sample> {
    println!(
        "\n Generating {} self-play games (GT Direct, boost={:.1})...",
        args.gen_games, args.boost
    );

    let mut gen_vs = nn::VarStore::new(device);
    let gen_net = GraphTransformerPolicyNet::new(
        &gen_vs,
        47,
        args.gen_embed_dim,
        args.gen_num_layers,
        args.gen_heads,
        0.0,
    );

    if !Path::new(&args.policy_path).exists() {
        eprintln!("Error: policy model not found: {}", args.policy_path);
        std::process::exit(1);
    }
    if let Err(e) = load_varstore(&mut gen_vs, &args.policy_path) {
        eprintln!("Error loading policy: {}", e);
        std::process::exit(1);
    }
    println!("   Loaded generator policy from {}", args.policy_path);

    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut samples = Vec::with_capacity(args.gen_games * 19);
    let mut score_sum = 0i64;
    let mut kept_games = 0usize;
    let gen_start = Instant::now();

    for game_idx in 0..args.gen_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut game_records: Vec<([i32; 19], (i32, i32, i32), usize, usize)> =
            Vec::with_capacity(19);

        let tile_seq = random_tile_sequence(&mut rng);

        for (turn, &tile) in tile_seq.iter().enumerate() {
            deck = replace_tile_in_deck(&deck, &tile);

            let mut plat_encoded = [0i32; 19];
            for i in 0..19 {
                let t = &plateau.tiles[i];
                if *t != Tile(0, 0, 0) {
                    plat_encoded[i] = t.0 * 100 + t.1 * 10 + t.2;
                }
            }

            let legal = get_legal_moves(&plateau);
            if legal.is_empty() {
                break;
            }

            let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            let feat_device = feat.unsqueeze(0).to_device(device);
            let logits = tch::no_grad(|| gen_net.forward(&feat_device, false))
                .squeeze_dim(0)
                .to_device(Device::Cpu);
            let logit_values: Vec<f64> = Vec::<f64>::try_from(&logits).unwrap();

            let pos = *legal
                .iter()
                .max_by(|&&a, &&b| {
                    let sa = logit_values[a] + line_boost(&plateau, &tile, a, args.boost);
                    let sb = logit_values[b] + line_boost(&plateau, &tile, b, args.boost);
                    sa.partial_cmp(&sb).unwrap()
                })
                .unwrap();

            game_records.push((plat_encoded, (tile.0, tile.1, tile.2), pos, turn));
            plateau.tiles[pos] = tile;
        }

        let final_score = result(&plateau);
        score_sum += final_score as i64;

        if final_score >= args.min_score {
            let weight = (final_score as f64 / 100.0).powf(args.weight_power);
            kept_games += 1;
            for (plat, tile, position, turn) in game_records {
                samples.push(Sample {
                    plateau: plat,
                    tile,
                    position,
                    turn,
                    final_score,
                    weight,
                });
            }
        }

        if (game_idx + 1) % 1000 == 0 {
            let avg = score_sum as f64 / (game_idx + 1) as f64;
            let elapsed = gen_start.elapsed().as_secs_f32();
            println!(
                "  Generated {}/{} | avg: {:.1} pts | kept: {} (>={}) | {:.0} g/s",
                game_idx + 1,
                args.gen_games,
                avg,
                kept_games,
                args.min_score,
                (game_idx + 1) as f64 / elapsed as f64
            );
        }
    }

    let avg = score_sum as f64 / args.gen_games as f64;
    println!(
        "   {} samples from {}/{} games (avg: {:.1} pts) in {:.1}s",
        samples.len(),
        kept_games,
        args.gen_games,
        avg,
        gen_start.elapsed().as_secs_f32()
    );
    samples
}

// -- CSV loading --------------------------------------------------------

fn load_all_csv_weighted(dir: &str, min_score: i32, weight_power: f64) -> Vec<Sample> {
    let mut samples = Vec::new();
    let path = Path::new(dir);
    if !path.exists() {
        return samples;
    }

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
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let mut samples = Vec::new();
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return samples,
    };
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
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
        let weight = (final_score as f64 / 100.0).powf(weight_power);

        samples.push(Sample {
            plateau,
            tile,
            position,
            turn,
            final_score,
            weight,
        });
    }
    samples
}

// -- Helpers ------------------------------------------------------------

fn decode_tile(encoded: i32) -> Tile {
    if encoded == 0 {
        return Tile(0, 0, 0);
    }
    Tile(encoded / 100, (encoded / 10) % 10, encoded % 10)
}

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

fn get_available_mask(sample: &Sample) -> Tensor {
    let mut mask = vec![f64::NEG_INFINITY; 19];
    for i in 0..19 {
        if sample.plateau[i] == 0 {
            mask[i] = 0.0;
        }
    }
    Tensor::from_slice(&mask)
}

fn compute_lr(base_lr: f64, epoch: usize, total: usize, scheduler: &str, min_ratio: f64) -> f64 {
    let min_lr = base_lr * min_ratio;
    match scheduler {
        "cosine" => {
            let progress = epoch as f64 / total as f64;
            min_lr + 0.5 * (base_lr - min_lr) * (1.0 + (std::f64::consts::PI * progress).cos())
        }
        _ => base_lr,
    }
}

fn evaluate_gpu(
    net: &SheafPolicyNet,
    features_gpu: &Tensor,
    targets_gpu: &Tensor,
    masks_gpu: &Tensor,
    indices: &[usize],
    batch_size: usize,
    device: Device,
) -> (f64, f64) {
    let n_batches = indices.len() / batch_size;
    if n_batches == 0 {
        return (0.0, 0.0);
    }

    let mut total_loss = 0.0;
    let mut total_correct = 0usize;

    for batch_i in 0..n_batches {
        let start = batch_i * batch_size;
        let end = start + batch_size;
        let batch_idx: Vec<i64> = indices[start..end].iter().map(|&i| i as i64).collect();
        let idx_tensor = Tensor::from_slice(&batch_idx).to_device(device);

        let features = features_gpu.index_select(0, &idx_tensor);
        let targets = targets_gpu.index_select(0, &idx_tensor);
        let masks = masks_gpu.index_select(0, &idx_tensor);

        let logits = tch::no_grad(|| net.forward(&features, false));
        let masked_logits = &logits + &masks;
        let log_probs = masked_logits.log_softmax(-1, Kind::Float);
        let loss = -log_probs
            .gather(1, &targets.unsqueeze(1), false)
            .squeeze_dim(1)
            .mean(Kind::Float);
        total_loss += f64::try_from(&loss).unwrap();

        let preds = masked_logits.argmax(-1, false);
        total_correct += preds.eq_tensor(&targets).sum(Kind::Int64).int64_value(&[]) as usize;
    }

    (
        total_loss / n_batches as f64,
        total_correct as f64 / (n_batches * batch_size) as f64,
    )
}

fn eval_games(
    net: &SheafPolicyNet,
    n_games: usize,
    rng: &mut StdRng,
    device: Device,
) -> (f64, Vec<i32>) {
    use take_it_easy::game::remove_tile_from_deck::get_available_tiles;

    let mut scores = Vec::new();
    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() {
                break;
            }
            let tile = *tiles.choose(rng).unwrap();

            let avail: Vec<usize> = (0..19)
                .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
                .collect();
            if avail.is_empty() {
                break;
            }

            let features = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            let feat_device = features.unsqueeze(0).to_device(device);
            let logits = tch::no_grad(|| net.forward(&feat_device, false))
                .squeeze_dim(0)
                .to_device(Device::Cpu);

            let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
            for &pos in &avail {
                let _ = mask.i(pos as i64).fill_(0.0);
            }
            let best_pos = (logits + mask).argmax(0, false).int64_value(&[]) as usize;

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        scores.push(result(&plateau));
    }
    (
        scores.iter().sum::<i32>() as f64 / scores.len() as f64,
        scores,
    )
}
