//! Distill V1Beam Strategy into Graph Transformer Weights
//!
//! 1. Play N games using GT+LineBoost+V1RowBonus as teacher (fast, center-9 aware)
//! 2. Record every (state, tile, position) as a supervised sample
//! 3. Fine-tune the GT model on these samples
//!
//! This "distills" the inference-time center-9 heuristic into the network weights,
//! eliminating the need for the heuristic at inference time.
//!
//! Usage: cargo run --release --bin distill_v1beam -- --num-games 10000

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::path::Path;
use std::time::Instant;
use tch::{nn, nn::OptimizerConfig, Device, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::remove_tile_from_deck::replace_tile_in_deck;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::graph_transformer::GraphTransformerPolicyNet;
use take_it_easy::neural::model_io::{load_varstore, save_varstore};
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "distill_v1beam")]
struct Args {
    /// Number of games to generate with teacher strategy
    #[arg(long, default_value_t = 10000)]
    num_games: usize,

    /// Training epochs over generated data
    #[arg(long, default_value_t = 20)]
    epochs: usize,

    /// Batch size
    #[arg(long, default_value_t = 64)]
    batch_size: usize,

    /// Learning rate
    #[arg(long, default_value_t = 0.00003)]
    lr: f64,

    /// Line boost strength for teacher
    #[arg(long, default_value_t = 3.0)]
    line_boost: f64,

    /// V1 bonus strength for teacher rollouts
    #[arg(long, default_value_t = 2.0)]
    v1_bonus: f64,

    /// Beam width for V1Beam teacher
    #[arg(long, default_value_t = 3)]
    beam_k: usize,

    /// Number of rollouts per beam candidate
    #[arg(long, default_value_t = 30)]
    beam_rollouts: usize,

    /// Use fast teacher (GT+LineBoost+V1Bonus argmax) instead of V1Beam with rollouts
    #[arg(long, default_value_t = false)]
    fast_teacher: bool,

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

    /// Load existing model weights (for both teacher and student init)
    #[arg(long, default_value = "model_weights/graph_transformer_policy.safetensors")]
    load_path: String,

    /// Save fine-tuned model path
    #[arg(long, default_value = "model_weights/gt_v1_distilled")]
    save_path: String,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Minimum teacher game score to include samples
    #[arg(long, default_value_t = 80)]
    min_score: i32,

    /// Weight multiplier for high-scoring teacher games (>= 150 pts)
    #[arg(long, default_value_t = 2.0)]
    high_score_weight: f64,

    /// Only apply v1 bonus for 9-tiles (surgical center-9 fix)
    #[arg(long, default_value_t = false)]
    nine_only: bool,
}

struct Sample {
    features: Tensor,
    target: i64,
    mask: Tensor,
    weight: f32,
}

/// Find best legal position in center row (7-11) for a 9-tile, if viable.
fn best_center_pos_for_9(plateau: &Plateau, tile: &Tile, legal: &[usize], gt_logits: &[f64]) -> Option<usize> {
    if tile.0 != 9 {
        return None;
    }
    const CENTER_ROW: &[usize] = &[7, 8, 9, 10, 11];

    // Check if center row is viable (no conflicting v1 placed)
    let viable = CENTER_ROW.iter().all(|&pos| {
        let t = &plateau.tiles[pos];
        *t == Tile(0, 0, 0) || t.0 == 9
    });
    if !viable {
        return None;
    }

    // Find legal positions in center row, pick one with highest GT logit
    let center_legal: Vec<usize> = legal.iter()
        .filter(|&&pos| (7..=11).contains(&pos))
        .copied()
        .collect();

    if center_legal.is_empty() {
        return None;
    }

    center_legal.iter()
        .max_by(|&&a, &&b| gt_logits[a].partial_cmp(&gt_logits[b]).unwrap())
        .copied()
}

/// Generate games: GT plays normally, but 9-tile labels are overridden to center row.
/// This keeps GT's optimal trajectory while surgically teaching center-9 placement.
fn generate_teacher_data(
    policy_net: &GraphTransformerPolicyNet,
    args: &Args,
    rng: &mut StdRng,
) -> (Vec<Sample>, Vec<i32>) {
    let mut all_samples = Vec::new();
    let mut game_scores = Vec::new();
    let mut overrides = 0usize;
    let mut total_nine_tiles = 0usize;
    let start = Instant::now();

    for game_i in 0..args.num_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut available: Vec<Tile> = deck
            .tiles()
            .iter()
            .copied()
            .filter(|t| *t != Tile(0, 0, 0))
            .collect();

        let mut game_samples = Vec::new();

        for turn in 0..19 {
            if available.is_empty() {
                break;
            }
            let tile_idx = rng.random_range(0..available.len());
            let tile = available.remove(tile_idx);
            deck = replace_tile_in_deck(&deck, &tile);

            let legal = get_legal_moves(&plateau);
            if legal.is_empty() {
                break;
            }

            // Compute features
            let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);

            // GT Direct chooses position (this is what the model actually plays)
            let logits = policy_net
                .forward(&feat.unsqueeze(0), false)
                .squeeze_dim(0);
            let mut mask_arr = [0.0f32; 19];
            for i in 0..19 {
                if plateau.tiles[i] != Tile(0, 0, 0) {
                    mask_arr[i] = f32::NEG_INFINITY;
                }
            }
            let masked = &logits + &Tensor::from_slice(&mask_arr);
            let gt_logit_values: Vec<f64> = Vec::<f64>::try_from(&masked).unwrap();
            let gt_pos = masked.argmax(-1, false).int64_value(&[]) as usize;

            // For 9-tiles: try to override label to center row
            let (target_pos, sample_weight_mult) = if tile.0 == 9 {
                total_nine_tiles += 1;
                if let Some(center_pos) = best_center_pos_for_9(&plateau, &tile, &legal, &gt_logit_values) {
                    if center_pos != gt_pos {
                        overrides += 1;
                        // Override: teach center placement, give higher weight
                        (center_pos, args.high_score_weight as f32)
                    } else {
                        // GT already chose center — reinforce
                        (gt_pos, 1.5f32)
                    }
                } else {
                    // Center not viable — keep GT's choice
                    (gt_pos, 1.0f32)
                }
            } else {
                // Non-9-tile: use GT's choice with normal weight
                (gt_pos, 1.0f32)
            };

            let mask_tensor = Tensor::from_slice(&mask_arr);
            game_samples.push((feat, target_pos as i64, mask_tensor, sample_weight_mult));

            // Game progresses with GT's actual choice (not the override)
            // This keeps the game trajectory optimal
            plateau.tiles[gt_pos] = tile;
        }

        let score = result(&plateau);
        game_scores.push(score);

        if score >= args.min_score {
            let base_weight = score as f32 / 100.0;
            for (feat, target, mask, weight_mult) in game_samples {
                all_samples.push(Sample {
                    features: feat,
                    target,
                    mask,
                    weight: base_weight * weight_mult,
                });
            }
        }

        if (game_i + 1) % 500 == 0 || game_i == args.num_games - 1 {
            let elapsed = start.elapsed().as_secs_f64();
            let recent_avg: f64 = game_scores
                .iter()
                .rev()
                .take(500)
                .map(|&s| s as f64)
                .sum::<f64>()
                / game_scores.len().min(500) as f64;
            print!(
                "\r  [{}/{}] samples={} overrides={}/{} avg={:.1} ({:.1}s)    ",
                game_i + 1,
                args.num_games,
                all_samples.len(),
                overrides,
                total_nine_tiles,
                recent_avg,
                elapsed
            );
        }
    }
    println!();

    (all_samples, game_scores)
}

/// Evaluate model by playing games with GT Direct (no heuristics).
fn eval_games(
    policy_net: &GraphTransformerPolicyNet,
    n_games: usize,
    rng: &mut StdRng,
) -> (f64, Vec<i32>) {
    let mut scores = Vec::with_capacity(n_games);

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut available: Vec<Tile> = deck
            .tiles()
            .iter()
            .copied()
            .filter(|t| *t != Tile(0, 0, 0))
            .collect();

        for turn in 0..19 {
            if available.is_empty() {
                break;
            }
            let tile_idx = rng.random_range(0..available.len());
            let tile = available.remove(tile_idx);
            deck = replace_tile_in_deck(&deck, &tile);

            let legal = get_legal_moves(&plateau);
            if legal.is_empty() {
                break;
            }

            let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19).unsqueeze(0);
            let logits = policy_net.forward(&feat, false).squeeze_dim(0);
            let mut mask = [0.0f32; 19];
            for i in 0..19 {
                if plateau.tiles[i] != Tile(0, 0, 0) {
                    mask[i] = f32::NEG_INFINITY;
                }
            }
            let masked = logits + Tensor::from_slice(&mask);
            let best_pos = masked.argmax(-1, false).int64_value(&[]) as usize;
            plateau.tiles[best_pos] = tile;
        }

        scores.push(result(&plateau));
    }

    let avg = scores.iter().sum::<i32>() as f64 / n_games as f64;
    (avg, scores)
}

/// Analyze 9-tile placement distribution of a model.
fn analyze_9tile_placement(
    policy_net: &GraphTransformerPolicyNet,
    n_games: usize,
    rng: &mut StdRng,
) -> (usize, usize, usize) {
    // Returns (center_count, edge_count, total_9tiles)
    let mut center = 0usize; // row 2 (positions 7-11)
    let mut edge = 0usize; // rows 0,4 (positions 0-2, 16-18)
    let mut total = 0usize;

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut available: Vec<Tile> = deck
            .tiles()
            .iter()
            .copied()
            .filter(|t| *t != Tile(0, 0, 0))
            .collect();

        for turn in 0..19 {
            if available.is_empty() {
                break;
            }
            let tile_idx = rng.random_range(0..available.len());
            let tile = available.remove(tile_idx);
            deck = replace_tile_in_deck(&deck, &tile);

            let legal = get_legal_moves(&plateau);
            if legal.is_empty() {
                break;
            }

            let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19).unsqueeze(0);
            let logits = policy_net.forward(&feat, false).squeeze_dim(0);
            let mut mask = [0.0f32; 19];
            for i in 0..19 {
                if plateau.tiles[i] != Tile(0, 0, 0) {
                    mask[i] = f32::NEG_INFINITY;
                }
            }
            let masked = logits + Tensor::from_slice(&mask);
            let best_pos = masked.argmax(-1, false).int64_value(&[]) as usize;

            // Track 9-tile placement
            if tile.0 == 9 {
                total += 1;
                match best_pos {
                    7..=11 => center += 1,
                    0..=2 | 16..=18 => edge += 1,
                    _ => {} // rows 1 or 3
                }
            }

            plateau.tiles[best_pos] = tile;
        }
    }

    (center, edge, total)
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     Distill V1Beam Strategy into GT Weights                  ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let teacher_type = if args.fast_teacher {
        "Fast (GT+LineBoost+V1Bonus argmax)"
    } else {
        "V1Beam (beam + v1-aware rollouts)"
    };

    println!("Config:");
    println!("  Teacher:          {}", teacher_type);
    println!("  Games to play:    {}", args.num_games);
    println!("  Line boost:       {:.1}", args.line_boost);
    println!("  V1 bonus:         {:.1}", args.v1_bonus);
    if !args.fast_teacher {
        println!("  Beam K:           {}", args.beam_k);
        println!("  Beam rollouts:    {}", args.beam_rollouts);
    }
    println!("  Nine-only mode:   {}", args.nine_only);
    println!("  Min score filter: {}", args.min_score);
    println!("  Epochs:           {}", args.epochs);
    println!("  Learning rate:    {}", args.lr);
    println!("  Batch size:       {}", args.batch_size);
    println!("  Load from:        {}", args.load_path);

    // Initialize model
    let device = Device::Cpu;
    let mut vs = nn::VarStore::new(device);
    let policy_net = GraphTransformerPolicyNet::new(
        &vs,
        47,
        args.embed_dim,
        args.num_layers,
        args.heads,
        args.dropout,
    );

    if Path::new(&args.load_path).exists() {
        match load_varstore(&mut vs, &args.load_path) {
            Ok(()) => println!("\n  Loaded weights from {}", args.load_path),
            Err(e) => {
                eprintln!("  Failed to load weights: {}", e);
                return;
            }
        }
    } else {
        eprintln!("  Model weights not found: {}", args.load_path);
        return;
    }

    // Baseline evaluation
    println!("\n--- Baseline (before distillation) ---");
    let mut rng = StdRng::seed_from_u64(args.seed);
    let (baseline_avg, baseline_scores) = eval_games(&policy_net, 300, &mut rng);
    let (b_center, b_edge, b_total) = analyze_9tile_placement(&policy_net, 300, &mut rng);
    println!("  GT Direct avg: {:.1} pts (300 games)", baseline_avg);
    println!(
        "  9-tile placement: center={:.1}% edge={:.1}% (n={})",
        b_center as f64 / b_total as f64 * 100.0,
        b_edge as f64 / b_total as f64 * 100.0,
        b_total
    );
    let above_150 = baseline_scores.iter().filter(|&&s| s >= 150).count();
    println!(
        "  Games >= 150: {} ({:.1}%)",
        above_150,
        above_150 as f64 / 300.0 * 100.0
    );

    // Step 1: Generate teacher data
    println!("\n--- Step 1: Generating teacher data ({} games) ---", args.num_games);
    let mut rng = StdRng::seed_from_u64(args.seed);
    let (samples, game_scores) = generate_teacher_data(&policy_net, &args, &mut rng);

    let teacher_avg: f64 =
        game_scores.iter().sum::<i32>() as f64 / game_scores.len() as f64;
    let kept_games = game_scores.iter().filter(|&&s| s >= args.min_score).count();
    println!("\n  Teacher avg score: {:.1} pts", teacher_avg);
    println!(
        "  Games kept (>= {} pts): {} / {}",
        args.min_score, kept_games, args.num_games
    );
    println!("  Training samples: {}", samples.len());

    if samples.is_empty() {
        println!("  No samples generated! Check min_score filter.");
        return;
    }

    // Step 2: Fine-tune
    println!("\n--- Step 2: Fine-tuning GT ({} epochs) ---\n", args.epochs);

    let mut opt = nn::Adam::default().build(&vs, args.lr).unwrap();
    let mut rng = StdRng::seed_from_u64(args.seed + 1);

    let n_samples = samples.len();
    let val_size = (n_samples as f64 * 0.05) as usize;
    let mut indices: Vec<usize> = (0..n_samples).collect();
    indices.shuffle(&mut rng);
    let (val_idx, train_idx) = indices.split_at(val_size);
    let val_indices: Vec<usize> = val_idx.to_vec();
    let train_indices: Vec<usize> = train_idx.to_vec();

    println!(
        "  Train: {} | Val: {}",
        train_indices.len(),
        val_indices.len()
    );

    let mut best_game_score = baseline_avg;
    let mut best_epoch = 0;

    for epoch in 0..args.epochs {
        let epoch_start = Instant::now();
        let mut shuffled = train_indices.clone();
        shuffled.shuffle(&mut rng);

        let n_batches = shuffled.len() / args.batch_size;
        if n_batches == 0 {
            println!("Not enough samples for a batch!");
            return;
        }

        let mut train_loss = 0.0;
        let mut train_correct = 0usize;

        for batch_i in 0..n_batches {
            let batch_start = batch_i * args.batch_size;
            let batch_end = batch_start + args.batch_size;
            let batch_idx = &shuffled[batch_start..batch_end];

            let features: Vec<Tensor> = batch_idx
                .iter()
                .map(|&i| samples[i].features.shallow_clone())
                .collect();
            let targets: Vec<i64> = batch_idx.iter().map(|&i| samples[i].target).collect();
            let masks: Vec<Tensor> = batch_idx
                .iter()
                .map(|&i| samples[i].mask.shallow_clone())
                .collect();
            let weights: Vec<f32> = batch_idx.iter().map(|&i| samples[i].weight).collect();

            let feat_tensor = Tensor::stack(&features, 0);
            let target_tensor = Tensor::from_slice(&targets);
            let mask_tensor = Tensor::stack(&masks, 0);
            let weight_tensor = Tensor::from_slice(&weights);

            let logits = policy_net.forward(&feat_tensor, true);
            let masked_logits = logits + &mask_tensor;
            let log_probs = masked_logits.log_softmax(-1, Kind::Float);

            let per_sample_loss =
                -log_probs.gather(1, &target_tensor.unsqueeze(1), false).squeeze_dim(1);
            let weighted_loss =
                (&per_sample_loss * &weight_tensor).sum(Kind::Float)
                    / weight_tensor.sum(Kind::Float);

            opt.backward_step(&weighted_loss);
            train_loss += f64::try_from(&weighted_loss).unwrap();

            let preds = masked_logits.argmax(-1, false);
            let correct: i64 = preds.eq_tensor(&target_tensor).sum(Kind::Int64).int64_value(&[]);
            train_correct += correct as usize;
        }

        train_loss /= n_batches as f64;
        let train_acc = train_correct as f64 / (n_batches * args.batch_size) as f64;

        // Validation accuracy
        let val_correct = {
            let mut correct = 0usize;
            let val_batches = val_indices.len() / args.batch_size;
            for batch_i in 0..val_batches {
                let batch_start = batch_i * args.batch_size;
                let batch_end = batch_start + args.batch_size;
                let batch_idx = &val_indices[batch_start..batch_end];

                let features: Vec<Tensor> = batch_idx
                    .iter()
                    .map(|&i| samples[i].features.shallow_clone())
                    .collect();
                let targets: Vec<i64> = batch_idx.iter().map(|&i| samples[i].target).collect();
                let masks: Vec<Tensor> = batch_idx
                    .iter()
                    .map(|&i| samples[i].mask.shallow_clone())
                    .collect();

                let feat_tensor = Tensor::stack(&features, 0);
                let target_tensor = Tensor::from_slice(&targets);
                let mask_tensor = Tensor::stack(&masks, 0);

                let logits = policy_net.forward(&feat_tensor, false);
                let masked_logits = logits + &mask_tensor;
                let preds = masked_logits.argmax(-1, false);
                let c: i64 = preds.eq_tensor(&target_tensor).sum(Kind::Int64).int64_value(&[]);
                correct += c as usize;
            }
            if val_batches > 0 {
                correct as f64 / (val_batches * args.batch_size) as f64
            } else {
                0.0
            }
        };

        let elapsed = epoch_start.elapsed().as_secs_f32();

        // Game evaluation every 5 epochs or last
        let should_eval = epoch % 5 == 4 || epoch == args.epochs - 1;
        let (game_score, nine_info) = if should_eval {
            let (avg, _scores) = eval_games(&policy_net, 200, &mut rng);
            let (center, edge, total) = analyze_9tile_placement(&policy_net, 200, &mut rng);
            (
                avg,
                format!(
                    " | 9→center={:.0}% edge={:.0}%",
                    center as f64 / total as f64 * 100.0,
                    edge as f64 / total as f64 * 100.0
                ),
            )
        } else {
            (0.0, String::new())
        };

        let improved = should_eval && game_score > best_game_score;

        if epoch % 2 == 0 || epoch == args.epochs - 1 || improved {
            let game_info = if should_eval {
                format!(" | Game: {:.1} pts", game_score)
            } else {
                String::new()
            };
            println!(
                "Epoch {:3}/{:3} | Loss: {:.4} Acc: {:.1}% | Val: {:.1}% | {:.1}s{}{}{}",
                epoch + 1,
                args.epochs,
                train_loss,
                train_acc * 100.0,
                val_correct * 100.0,
                elapsed,
                game_info,
                nine_info,
                if improved { " *" } else { "" }
            );
        }

        if improved {
            best_game_score = game_score;
            best_epoch = epoch + 1;
            let path = format!("{}_policy.safetensors", args.save_path);
            if let Err(e) = save_varstore(&vs, &path) {
                eprintln!("Warning: failed to save: {}", e);
            } else {
                println!("   Saved to {}", path);
            }
        }
    }

    // Final evaluation
    println!("\n══════════════════════════════════════════════════════════════");
    println!("                     DISTILLATION COMPLETE");
    println!("══════════════════════════════════════════════════════════════");
    println!("\n  Baseline GT Direct:    {:.1} pts", baseline_avg);
    println!(
        "  Best distilled score:  {:.1} pts (epoch {})",
        best_game_score, best_epoch
    );
    println!(
        "  Delta:                 {:+.1} pts",
        best_game_score - baseline_avg
    );

    // Detailed final eval
    println!("\n--- Final evaluation (500 games) ---");
    let mut rng = StdRng::seed_from_u64(args.seed + 99);
    let (final_avg, final_scores) = eval_games(&policy_net, 500, &mut rng);
    let (f_center, f_edge, f_total) = analyze_9tile_placement(&policy_net, 500, &mut rng);

    let above_100 = final_scores.iter().filter(|&&s| s >= 100).count();
    let above_140 = final_scores.iter().filter(|&&s| s >= 140).count();
    let above_150 = final_scores.iter().filter(|&&s| s >= 150).count();
    let min = final_scores.iter().min().unwrap();
    let max = final_scores.iter().max().unwrap();

    println!("  GT Direct (distilled): {:.1} pts", final_avg);
    println!("  Min: {} | Max: {}", min, max);
    println!(
        "  >= 100: {} ({:.1}%) | >= 140: {} ({:.1}%) | >= 150: {} ({:.1}%)",
        above_100,
        above_100 as f64 / 5.0,
        above_140,
        above_140 as f64 / 5.0,
        above_150,
        above_150 as f64 / 5.0
    );
    println!(
        "\n  9-tile placement: center={:.1}% edge={:.1}% (n={})",
        f_center as f64 / f_total as f64 * 100.0,
        f_edge as f64 / f_total as f64 * 100.0,
        f_total
    );
    println!(
        "  (Baseline was:    center={:.1}% edge={:.1}%)",
        b_center as f64 / b_total as f64 * 100.0,
        b_edge as f64 / b_total as f64 * 100.0
    );
}
