//! Distill Expectimax Hybrid decisions into a new policy network.
//!
//! 1. Play games with GT+Ex(t>=min_turn) hybrid strategy
//! 2. Record (features, chosen_position, final_score) at each turn
//! 3. Train a new policy net via cross-entropy on these decisions
//! 4. The distilled policy reproduces expectimax-quality moves with zero runtime overhead
//!
//! Usage:
//!   cargo run --release --bin distill_expectimax -- --device cuda --num-games 100000

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::path::Path;
use std::time::Instant;
use tch::{nn, nn::OptimizerConfig, Device, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::create_plateau_empty;
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
use take_it_easy::strategy::gt_boost::line_boost;

#[derive(Parser, Debug)]
#[command(name = "distill_expectimax")]
#[command(about = "Distill GT+Expectimax hybrid into a pure policy network")]
struct Args {
    #[arg(long, default_value = "cpu")]
    device: String,

    /// Number of games to generate training data
    #[arg(long, default_value_t = 100000)]
    num_games: usize,

    /// Training epochs
    #[arg(long, default_value_t = 80)]
    epochs: usize,

    /// Learning rate
    #[arg(long, default_value_t = 0.0005)]
    lr: f64,

    /// Batch size
    #[arg(long, default_value_t = 64)]
    batch_size: usize,

    /// Score weighting power: weight = (score/100)^power
    #[arg(long, default_value_t = 3.0)]
    weight_power: f64,

    /// Line-boost strength
    #[arg(long, default_value_t = 3.0)]
    boost: f64,

    /// Expectimax min_turn (GT Direct for turns < min_turn)
    #[arg(long, default_value_t = 8)]
    min_turn: usize,

    /// Path to teacher policy model
    #[arg(long, default_value = "model_weights/graph_transformer_policy.safetensors")]
    policy_path: String,

    /// Path to value network (teacher)
    #[arg(long, default_value = "model_weights/value_net_100k.safetensors")]
    value_path: String,

    /// Path to save distilled policy
    #[arg(long, default_value = "model_weights/distilled_expectimax_policy.safetensors")]
    save_path: String,

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

    /// Weight decay
    #[arg(long, default_value_t = 0.0001)]
    weight_decay: f64,

    /// Min LR ratio for cosine schedule
    #[arg(long, default_value_t = 0.01)]
    min_lr_ratio: f64,

    /// Validation split
    #[arg(long, default_value_t = 0.1)]
    val_split: f64,

    /// Number of evaluation games
    #[arg(long, default_value_t = 500)]
    eval_games: usize,

    /// Score normalization mean (for expectimax config)
    #[arg(long, default_value_t = 140.0)]
    score_mean: f64,

    /// Score normalization std (for expectimax config)
    #[arg(long, default_value_t = 40.0)]
    score_std: f64,

    /// Initialize student from teacher weights (fine-tune) vs random init
    #[arg(long, default_value_t = true)]
    init_from_teacher: bool,
}

struct Sample {
    features: Tensor,    // [19, 47]
    position: usize,     // chosen position (0-18)
    mask: Vec<f64>,      // legal move mask (0.0 or -inf)
    weight: f64,         // score-based weight
}

fn main() {
    let args = Args::parse();

    println!("================================================");
    println!("  Expectimax Policy Distillation");
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
    println!("Strategy: GT Direct (t<{}) + Expectimax (t>={})", args.min_turn, args.min_turn);

    // ── Load teacher models ──
    let mut teacher_policy_vs = nn::VarStore::new(device);
    let teacher_policy = GraphTransformerPolicyNet::new(
        &teacher_policy_vs, 47, args.embed_dim, args.num_layers, args.num_heads, 0.0,
    );
    if let Err(e) = load_varstore(&mut teacher_policy_vs, &args.policy_path) {
        eprintln!("Error loading policy: {}", e);
        return;
    }
    println!("Loaded teacher policy from {}", args.policy_path);

    let mut value_vs = nn::VarStore::new(device);
    let value_net = GraphTransformerValueNet::new(
        &value_vs, 47, args.embed_dim, args.num_layers, args.num_heads, 0.0,
    );
    if let Err(e) = load_varstore(&mut value_vs, &args.value_path) {
        eprintln!("Error loading value net: {}", e);
        return;
    }
    println!("Loaded value net from {}", args.value_path);

    // ── Phase 1: Generate data with hybrid strategy ──
    println!("\n--- Phase 1: Data Generation ({} games) ---\n", args.num_games);

    let ex_config = ExpectimaxConfig {
        device,
        boost: args.boost,
        score_mean: args.score_mean,
        score_std: args.score_std,
        min_turn: args.min_turn,
    };

    let gen_start = Instant::now();
    let samples = generate_data(&teacher_policy, &value_net, &ex_config, device, &args);
    let gen_time = gen_start.elapsed().as_secs_f32();

    let scores: Vec<f64> = samples.iter().map(|s| s.weight).collect();
    let total_weight: f64 = scores.iter().sum();
    println!("{} samples ready ({:.1}s)", samples.len(), gen_time);
    println!("Total weight: {:.0}, avg weight: {:.2}", total_weight, total_weight / samples.len() as f64);

    // ── Phase 2: Train student policy ──
    println!("\n--- Phase 2: Training ---\n");

    let student_vs = nn::VarStore::new(device);
    let student_policy = GraphTransformerPolicyNet::new(
        &student_vs, 47, args.embed_dim, args.num_layers, args.num_heads, args.dropout,
    );

    // Initialize from teacher weights
    if args.init_from_teacher && Path::new(&args.policy_path).exists() {
        // Load teacher weights into student
        let mut init_vs = nn::VarStore::new(device);
        let _init_net = GraphTransformerPolicyNet::new(
            &init_vs, 47, args.embed_dim, args.num_layers, args.num_heads, args.dropout,
        );
        if load_varstore(&mut init_vs, &args.policy_path).is_ok() {
            // Copy weights variable by variable
            let teacher_vars = init_vs.variables();
            let mut student_vars = student_vs.variables();
            let mut copied = 0;
            for (name, tensor) in &teacher_vars {
                if let Some(student_tensor) = student_vars.get_mut(name) {
                    tch::no_grad(|| student_tensor.copy_(tensor));
                    copied += 1;
                }
            }
            println!("Initialized student from teacher ({}/{} vars)", copied, teacher_vars.len());
        }
    }

    let mut opt = nn::Adam {
        wd: args.weight_decay,
        ..Default::default()
    }
    .build(&student_vs, args.lr)
    .unwrap();

    // Split train/val
    let mut rng = StdRng::seed_from_u64(args.seed + 999);
    let mut indices: Vec<usize> = (0..samples.len()).collect();
    indices.shuffle(&mut rng);

    let val_size = (samples.len() as f64 * args.val_split) as usize;
    let val_indices: Vec<usize> = indices[..val_size].to_vec();
    let train_indices: Vec<usize> = indices[val_size..].to_vec();
    println!("Train: {} samples, Val: {}", train_indices.len(), val_indices.len());

    let train_start = Instant::now();
    let mut best_val_loss = f64::INFINITY;
    let mut best_val_acc = 0.0;

    for epoch in 0..args.epochs {
        let epoch_start = Instant::now();
        let lr = compute_lr(args.lr, epoch, args.epochs, args.min_lr_ratio);
        opt.set_lr(lr);

        // Training
        let mut train_loss_sum = 0.0;
        let mut train_correct = 0usize;
        let mut train_count = 0usize;

        let mut train_perm = train_indices.clone();
        train_perm.shuffle(&mut rng);

        for batch_start in (0..train_perm.len()).step_by(args.batch_size) {
            let batch_end = (batch_start + args.batch_size).min(train_perm.len());
            let batch_idx = &train_perm[batch_start..batch_end];

            let (features, targets, masks, weights) = prepare_batch(&samples, batch_idx, device);

            let logits = student_policy.forward(&features, true);
            let masked_logits = logits + &masks;
            let log_probs = masked_logits.log_softmax(-1, Kind::Float);
            let per_sample_loss = -log_probs
                .gather(1, &targets.unsqueeze(1), false)
                .squeeze_dim(1);
            let weighted_loss =
                (&per_sample_loss * &weights).sum(Kind::Float) / weights.sum(Kind::Float);
            opt.backward_step(&weighted_loss);

            let n = batch_idx.len();
            train_loss_sum += weighted_loss.double_value(&[]) * n as f64;

            let preds = masked_logits.argmax(-1, false);
            train_correct += preds.eq_tensor(&targets).sum(Kind::Int64).int64_value(&[]) as usize;
            train_count += n;
        }

        let train_loss = train_loss_sum / train_count as f64;
        let train_acc = train_correct as f64 / train_count as f64 * 100.0;

        // Validation
        let mut val_loss_sum = 0.0;
        let mut val_correct = 0usize;
        let mut val_count = 0usize;

        for batch_start in (0..val_indices.len()).step_by(args.batch_size) {
            let batch_end = (batch_start + args.batch_size).min(val_indices.len());
            let batch_idx = &val_indices[batch_start..batch_end];

            let (features, targets, masks, weights) = prepare_batch(&samples, batch_idx, device);

            let logits = tch::no_grad(|| student_policy.forward(&features, false));
            let masked_logits = &logits + &masks;
            let log_probs = masked_logits.log_softmax(-1, Kind::Float);
            let per_sample_loss = -log_probs
                .gather(1, &targets.unsqueeze(1), false)
                .squeeze_dim(1);
            let loss = (&per_sample_loss * &weights).sum(Kind::Float) / weights.sum(Kind::Float);

            let n = batch_idx.len();
            val_loss_sum += loss.double_value(&[]) * n as f64;

            let preds = masked_logits.argmax(-1, false);
            val_correct += preds.eq_tensor(&targets).sum(Kind::Int64).int64_value(&[]) as usize;
            val_count += n;
        }

        let val_loss = val_loss_sum / val_count as f64;
        let val_acc = val_correct as f64 / val_count as f64 * 100.0;
        let epoch_time = epoch_start.elapsed().as_secs_f32();

        let saved = if val_loss < best_val_loss {
            best_val_loss = val_loss;
            best_val_acc = val_acc;
            if let Err(e) = save_varstore(&student_vs, &args.save_path) {
                eprintln!("Warning: failed to save: {}", e);
            }
            true
        } else {
            false
        };

        if epoch % 5 == 0 || saved || epoch == args.epochs - 1 {
            print!(
                "Epoch {:3}/{:3} | Train L:{:.4} Acc:{:.1}% | Val L:{:.4} Acc:{:.1}% | {:.1}s",
                epoch + 1, args.epochs, train_loss, train_acc, val_loss, val_acc, epoch_time
            );
            if saved {
                print!(" *");
            }
            println!();
        }
    }

    let train_time = train_start.elapsed().as_secs_f32();
    println!("\nTraining complete in {:.1}s", train_time);
    println!("Best val loss: {:.4}, val acc: {:.1}%", best_val_loss, best_val_acc);
    println!("Model saved to: {}", args.save_path);

    // ── Phase 3: Evaluate distilled policy ──
    if args.eval_games > 0 {
        println!("\n--- Phase 3: Evaluation ({} games) ---\n", args.eval_games);

        // Reload best student weights
        let mut eval_student_vs = nn::VarStore::new(device);
        let eval_student = GraphTransformerPolicyNet::new(
            &eval_student_vs, 47, args.embed_dim, args.num_layers, args.num_heads, 0.0,
        );
        if let Err(e) = load_varstore(&mut eval_student_vs, &args.save_path) {
            eprintln!("Error reloading student: {}", e);
            return;
        }

        let mut eval_rng = StdRng::seed_from_u64(args.seed + 5000);
        let eval_sequences: Vec<Vec<Tile>> = (0..args.eval_games)
            .map(|_| random_tile_sequence(&mut eval_rng))
            .collect();

        // GT Direct (teacher)
        print!("  GT Direct (teacher)...");
        std::io::Write::flush(&mut std::io::stdout()).ok();
        let gt_scores: Vec<i32> = eval_sequences
            .iter()
            .map(|seq| play_gt_direct(&teacher_policy, device, seq, args.boost))
            .collect();
        let gt_avg = avg(&gt_scores);
        println!(" {:.1} pts", gt_avg);

        // GT+Expectimax hybrid (teacher)
        print!("  GT+Ex(t>={}) (teacher)...", args.min_turn);
        std::io::Write::flush(&mut std::io::stdout()).ok();
        let ex_scores: Vec<i32> = eval_sequences
            .iter()
            .map(|seq| play_expectimax_game(&teacher_policy, &value_net, &ex_config, seq))
            .collect();
        let ex_avg = avg(&ex_scores);
        println!(" {:.1} pts ({:+.1})", ex_avg, ex_avg - gt_avg);

        // Distilled student (GT Direct mode — no value net needed!)
        print!("  Distilled policy...");
        std::io::Write::flush(&mut std::io::stdout()).ok();
        let distilled_scores: Vec<i32> = eval_sequences
            .iter()
            .map(|seq| play_gt_direct(&eval_student, device, seq, args.boost))
            .collect();
        let dist_avg = avg(&distilled_scores);
        println!(" {:.1} pts ({:+.1})", dist_avg, dist_avg - gt_avg);

        // Summary
        println!("\n{}", "=".repeat(68));
        println!(
            "{:<24} {:>8} {:>8} {:>8} {:>8} {:>8}",
            "Strategy", "Avg", "Std", "Min", "Max", "Delta"
        );
        println!("{}", "-".repeat(68));
        println!(
            "{:<24} {:>8.1} {:>8.1} {:>8} {:>8}",
            "GT Direct (teacher)", gt_avg, std_dev(&gt_scores),
            gt_scores.iter().min().unwrap(), gt_scores.iter().max().unwrap()
        );
        println!(
            "{:<24} {:>8.1} {:>8.1} {:>8} {:>8} {:>+8.1}",
            format!("GT+Ex(t>={})", args.min_turn), ex_avg, std_dev(&ex_scores),
            ex_scores.iter().min().unwrap(), ex_scores.iter().max().unwrap(),
            ex_avg - gt_avg
        );
        println!(
            "{:<24} {:>8.1} {:>8.1} {:>8} {:>8} {:>+8.1}",
            "Distilled policy", dist_avg, std_dev(&distilled_scores),
            distilled_scores.iter().min().unwrap(), distilled_scores.iter().max().unwrap(),
            dist_avg - gt_avg
        );
        println!("{}", "=".repeat(68));
    }
}

/// Generate training data by playing with GT+Expectimax hybrid.
fn generate_data(
    policy_net: &GraphTransformerPolicyNet,
    value_net: &GraphTransformerValueNet,
    ex_config: &ExpectimaxConfig,
    _device: Device,
    args: &Args,
) -> Vec<Sample> {
    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut samples = Vec::with_capacity(args.num_games * 19);
    let mut score_sum = 0i64;

    for game_idx in 0..args.num_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut game_samples: Vec<(Tensor, usize, Vec<f64>)> = Vec::with_capacity(19);

        let tile_seq = random_tile_sequence(&mut rng);

        for (turn, &tile) in tile_seq.iter().enumerate() {
            deck = replace_tile_in_deck(&deck, &tile);
            let legal = get_legal_moves(&plateau);
            if legal.is_empty() {
                break;
            }

            // Record features BEFORE the move
            let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);

            // Build mask
            let mut mask = vec![f64::NEG_INFINITY; 19];
            for &pos in &legal {
                mask[pos] = 0.0;
            }

            // Choose position with hybrid strategy
            let pos = if legal.len() == 1 {
                legal[0]
            } else {
                expectimax_select(&plateau, &tile, &deck, turn, policy_net, value_net, ex_config)
            };

            game_samples.push((feat, pos, mask));
            plateau.tiles[pos] = tile;
        }

        let final_score = result(&plateau);
        score_sum += final_score as i64;
        let weight = (final_score as f64 / 100.0).powf(args.weight_power);

        for (feat, pos, mask) in game_samples {
            samples.push(Sample {
                features: feat,
                position: pos,
                mask,
                weight,
            });
        }

        if (game_idx + 1) % 1000 == 0 {
            let avg_score = score_sum as f64 / (game_idx + 1) as f64;
            println!(
                "  Generated {}/{} games (avg: {:.1}, last: {})",
                game_idx + 1, args.num_games, avg_score, final_score
            );
        }
    }

    samples
}

fn prepare_batch(
    samples: &[Sample],
    indices: &[usize],
    device: Device,
) -> (Tensor, Tensor, Tensor, Tensor) {
    let features: Vec<Tensor> = indices
        .iter()
        .map(|&i| samples[i].features.shallow_clone())
        .collect();
    let targets: Vec<i64> = indices.iter().map(|&i| samples[i].position as i64).collect();
    let masks: Vec<Tensor> = indices
        .iter()
        .map(|&i| Tensor::from_slice(&samples[i].mask))
        .collect();
    let weights: Vec<f64> = indices.iter().map(|&i| samples[i].weight).collect();

    (
        Tensor::stack(&features, 0).to_device(device),
        Tensor::from_slice(&targets).to_device(device),
        Tensor::stack(&masks, 0).to_kind(Kind::Float).to_device(device),
        Tensor::from_slice(&weights)
            .to_kind(Kind::Float)
            .to_device(device),
    )
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

/// Play one game with expectimax hybrid strategy.
fn play_expectimax_game(
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

fn avg(scores: &[i32]) -> f64 {
    scores.iter().sum::<i32>() as f64 / scores.len() as f64
}

fn std_dev(scores: &[i32]) -> f64 {
    let mean = avg(scores);
    let var = scores.iter().map(|&s| (s as f64 - mean).powi(2)).sum::<f64>() / scores.len() as f64;
    var.sqrt()
}
