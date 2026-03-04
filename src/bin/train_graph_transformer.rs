//! Graph Transformer Training
//!
//! Trains a Graph Transformer with full self-attention between all 19 nodes.
//! Unlike GAT which only attends to neighbors, this allows learning long-range
//! dependencies across the board.
//!
//! Supports GPU training (--device cuda) and on-the-fly data generation
//! via GT Direct self-play (--gen-games N --policy-path <path>).
//!
//! Usage:
//!   cargo run --release --bin train_graph_transformer -- --epochs 80
//!   cargo run --release --bin train_graph_transformer -- --device cuda --gen-games 100000 \
//!     --policy-path model_weights/graph_transformer_policy.safetensors \
//!     --embed-dim 256 --num-layers 4 --heads 8 --dropout 0.2

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
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::remove_tile_from_deck::replace_tile_in_deck;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::device_util::{check_cuda, parse_device};
use take_it_easy::neural::graph_transformer::GraphTransformerPolicyNet;
use take_it_easy::neural::model_io::{load_varstore, save_varstore};
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;
use take_it_easy::strategy::gt_boost::line_boost;

#[derive(Parser, Debug)]
#[command(name = "train_graph_transformer")]
struct Args {
    /// Device: "cpu", "cuda", "cuda:0"
    #[arg(long, default_value = "cpu")]
    device: String,

    /// Minimum score to include (CSV mode)
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
    #[arg(long, default_value_t = 0.0005)]
    lr: f64,

    /// Embedding dimension
    #[arg(long, default_value_t = 128)]
    embed_dim: i64,

    /// Number of transformer layers
    #[arg(long, default_value_t = 2)]
    num_layers: usize,

    /// Number of attention heads
    #[arg(long, default_value_t = 4)]
    heads: i64,

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

    /// Save model path (without extension)
    #[arg(long, default_value = "model_weights/graph_transformer")]
    save_path: String,

    /// Data directory (CSV mode, used when gen-games == 0)
    #[arg(long, default_value = "data")]
    data_dir: String,

    /// Number of GT Direct self-play games to generate (0 = use CSV from data-dir)
    #[arg(long, default_value_t = 0)]
    gen_games: usize,

    /// Path to existing policy model for self-play generation
    #[arg(long, default_value = "model_weights/graph_transformer_policy.safetensors")]
    policy_path: String,

    /// Generator policy embed dim (must match policy-path model)
    #[arg(long, default_value_t = 128)]
    gen_embed_dim: i64,

    /// Generator policy layers (must match policy-path model)
    #[arg(long, default_value_t = 2)]
    gen_num_layers: usize,

    /// Generator policy heads (must match policy-path model)
    #[arg(long, default_value_t = 4)]
    gen_heads: i64,

    /// Line-boost strength for GT Direct self-play
    #[arg(long, default_value_t = 3.0)]
    boost: f64,
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

    let device = match parse_device(&args.device) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    };
    check_cuda();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       Graph Transformer Training                             ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("Config:");
    println!("  Device:       {:?}", device);
    println!("  Architecture: Graph Transformer (Full Attention)");
    println!("  Embed dim:    {}", args.embed_dim);
    println!("  Layers:       {}", args.num_layers);
    println!("  Heads:        {}", args.heads);
    println!("  FF dim:       {} (4x embed)", args.embed_dim * 4);
    println!("  Dropout:      {}", args.dropout);
    println!("  Weight decay: {}", args.weight_decay);
    println!("  Epochs:       {}", args.epochs);
    println!("  LR:           {} ({})", args.lr, args.lr_scheduler);
    println!("  Weight power: {:.1}", args.weight_power);
    if args.gen_games > 0 {
        println!("  Data:         {} self-play games (GT Direct)", args.gen_games);
        println!("  Policy:       {}", args.policy_path);
        println!("  Gen arch:     dim={}, layers={}, heads={}", args.gen_embed_dim, args.gen_num_layers, args.gen_heads);
        println!("  Boost:        {:.1}", args.boost);
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

    // Initialize network on target device
    let vs = nn::VarStore::new(device);
    let policy_net = GraphTransformerPolicyNet::new(
        &vs,
        47, // input_dim
        args.embed_dim,
        args.num_layers,
        args.heads,
        args.dropout,
    );
    let mut opt = nn::Adam {
        wd: args.weight_decay,
        ..Default::default()
    }.build(&vs, args.lr).unwrap();

    let mut best_val_acc = 0.0f64;
    let mut best_game_score = 0.0f64;

    // Training loop
    println!("\n Training Graph Transformer...\n");
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

            let (features, targets, masks, weights) = prepare_batch_weighted(&samples, &batch_indices, device);

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
        let (val_loss, val_acc) = evaluate(&policy_net, &samples, &val_indices, args.batch_size, device);

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
            let path = format!("{}_policy.safetensors", args.save_path);
            if let Err(e) = save_varstore(&vs, &path) {
                eprintln!("Warning: failed to save: {}", e);
            }
            println!("   New best game score! Model saved.");
        }
    }

    println!("\n══════════════════════════════════════════════════════════════");
    println!("                     TRAINING COMPLETE");
    println!("══════════════════════════════════════════════════════════════");
    println!("\n  Architecture: Graph Transformer");
    println!("  Best validation accuracy: {:.2}%", best_val_acc * 100.0);
    println!("  Best game score: {:.2} pts", best_game_score);

    // Final evaluation
    println!("\n Evaluating by playing 200 games...\n");
    let (gt_avg, gt_scores) = eval_games(&policy_net, 200, &mut rng, device);
    let greedy_avg = eval_greedy(200, args.seed);

    println!("  Graph Transformer: {:.2} pts", gt_avg);
    println!("  Greedy:            {:.2} pts", greedy_avg);
    println!("\n  vs Greedy: {:+.2} pts", gt_avg - greedy_avg);

    let above_100: usize = gt_scores.iter().filter(|&&s| s >= 100).count();
    let above_140: usize = gt_scores.iter().filter(|&&s| s >= 140).count();
    let above_150: usize = gt_scores.iter().filter(|&&s| s >= 150).count();
    println!("\n  Games >= 100 pts: {} ({:.1}%)", above_100, above_100 as f64 / 200.0 * 100.0);
    println!("  Games >= 140 pts: {} ({:.1}%)", above_140, above_140 as f64 / 200.0 * 100.0);
    println!("  Games >= 150 pts: {} ({:.1}%)", above_150, above_150 as f64 / 200.0 * 100.0);
}

// ── Self-play data generation ─────────────────────────────────────────────

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
        if available.is_empty() { break; }
        let idx = rng.random_range(0..available.len());
        seq.push(available.remove(idx));
    }
    seq
}

fn generate_selfplay_data(args: &Args, device: Device) -> Vec<Sample> {
    println!("\n Generating {} self-play games (GT Direct, boost={:.1})...", args.gen_games, args.boost);

    // Load policy net for self-play on target device (use generator dims, not student dims)
    let mut gen_vs = nn::VarStore::new(device);
    let gen_net = GraphTransformerPolicyNet::new(
        &gen_vs, 47, args.gen_embed_dim, args.gen_num_layers, args.gen_heads, 0.0,
    );

    if !Path::new(&args.policy_path).exists() {
        eprintln!("Error: policy model not found: {}", args.policy_path);
        std::process::exit(1);
    }
    match load_varstore(&mut gen_vs, &args.policy_path) {
        Ok(()) => println!("   Loaded policy from {}", args.policy_path),
        Err(e) => {
            eprintln!("Error loading policy: {}", e);
            std::process::exit(1);
        }
    }

    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut samples = Vec::with_capacity(args.gen_games * 19);
    let mut score_sum = 0i64;
    let gen_start = Instant::now();

    for game_idx in 0..args.gen_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut game_records: Vec<([i32; 19], (i32, i32, i32), usize, usize)> = Vec::with_capacity(19);

        let tile_seq = random_tile_sequence(&mut rng);

        for (turn, &tile) in tile_seq.iter().enumerate() {
            deck = replace_tile_in_deck(&deck, &tile);

            // Record plateau state before move
            let mut plat_encoded = [0i32; 19];
            for i in 0..19 {
                let t = &plateau.tiles[i];
                if *t != Tile(0, 0, 0) {
                    plat_encoded[i] = t.0 * 100 + t.1 * 10 + t.2;
                }
            }
            let tile_encoded = (tile.0, tile.1, tile.2);

            let legal = get_legal_moves(&plateau);
            if legal.is_empty() { break; }

            // GT Direct inference on device
            let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            let feat_device = feat.unsqueeze(0).to_device(device);
            let logits = tch::no_grad(|| gen_net.forward(&feat_device, false))
                .squeeze_dim(0)
                .to_device(Device::Cpu);
            let logit_values: Vec<f64> = Vec::<f64>::try_from(&logits).unwrap();

            // Mask + line_boost -> argmax
            let pos = *legal
                .iter()
                .max_by(|&&a, &&b| {
                    let sa = logit_values[a] + line_boost(&plateau, &tile, a, args.boost);
                    let sb = logit_values[b] + line_boost(&plateau, &tile, b, args.boost);
                    sa.partial_cmp(&sb).unwrap()
                })
                .unwrap();

            game_records.push((plat_encoded, tile_encoded, pos, turn));
            plateau.tiles[pos] = tile;
        }

        let final_score = result(&plateau);
        score_sum += final_score as i64;
        let weight = (final_score as f64 / 100.0).powf(args.weight_power);

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

        if (game_idx + 1) % 1000 == 0 {
            let avg = score_sum as f64 / (game_idx + 1) as f64;
            let elapsed = gen_start.elapsed().as_secs_f32();
            let games_per_sec = (game_idx + 1) as f64 / elapsed as f64;
            println!("  Generated {}/{} games | avg: {:.1} pts | {:.0} games/s",
                     game_idx + 1, args.gen_games, avg, games_per_sec);
        }
    }

    let avg = score_sum as f64 / args.gen_games as f64;
    let elapsed = gen_start.elapsed().as_secs_f32();
    println!("   {} samples from {} games (avg: {:.1} pts) in {:.1}s",
             samples.len(), args.gen_games, avg, elapsed);

    samples
}

// ── CSV loading ───────────────────────────────────────────────────────────

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

// ── Training helpers ──────────────────────────────────────────────────────

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

fn prepare_batch_weighted(samples: &[Sample], indices: &[usize], device: Device) -> (Tensor, Tensor, Tensor, Tensor) {
    let features: Vec<Tensor> = indices.iter().map(|&i| sample_to_features(&samples[i])).collect();
    let targets: Vec<i64> = indices.iter().map(|&i| samples[i].position as i64).collect();
    let masks: Vec<Tensor> = indices.iter().map(|&i| get_available_mask(&samples[i])).collect();
    let weights: Vec<f64> = indices.iter().map(|&i| samples[i].weight).collect();

    (
        Tensor::stack(&features, 0).to_device(device),
        Tensor::from_slice(&targets).to_device(device),
        Tensor::stack(&masks, 0).to_device(device),
        Tensor::from_slice(&weights).to_kind(Kind::Float).to_device(device),
    )
}

fn evaluate(net: &GraphTransformerPolicyNet, samples: &[Sample], indices: &[usize], batch_size: usize, device: Device) -> (f64, f64) {
    let n_batches = indices.len() / batch_size;
    if n_batches == 0 { return (0.0, 0.0); }

    let mut total_loss = 0.0;
    let mut total_correct = 0usize;

    for batch_i in 0..n_batches {
        let batch_indices: Vec<usize> = indices[batch_i * batch_size..(batch_i + 1) * batch_size].to_vec();
        let (features, targets, masks, _) = prepare_batch_weighted(samples, &batch_indices, device);

        let logits = tch::no_grad(|| net.forward(&features, false));
        let masked_logits = &logits + &masks;
        let log_probs = masked_logits.log_softmax(-1, Kind::Float);
        let loss = -log_probs.gather(1, &targets.unsqueeze(1), false).squeeze_dim(1).mean(Kind::Float);
        total_loss += f64::try_from(&loss).unwrap();

        let preds = masked_logits.argmax(-1, false);
        total_correct += preds.eq_tensor(&targets).sum(Kind::Int64).int64_value(&[]) as usize;
    }

    (total_loss / n_batches as f64, total_correct as f64 / (n_batches * batch_size) as f64)
}

fn eval_games(policy_net: &GraphTransformerPolicyNet, n_games: usize, rng: &mut StdRng, device: Device) -> (f64, Vec<i32>) {
    use take_it_easy::game::remove_tile_from_deck::get_available_tiles;

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
            let feat_device = features.unsqueeze(0).to_device(device);
            let logits = tch::no_grad(|| policy_net.forward(&feat_device, false))
                .squeeze_dim(0)
                .to_device(Device::Cpu);

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
