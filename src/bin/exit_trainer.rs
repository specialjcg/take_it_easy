//! Expert Iteration (ExIt) Trainer for Take It Easy
//!
//! Uses V1Beam (with rollouts) as "expert" to generate high-quality games,
//! filters the top percentile, re-trains the Graph Transformer, then iterates.
//! Each iteration the GT improves → V1Beam improves → data improves → virtuous cycle.
//!
//! Usage: cargo run --release --bin exit_trainer -- --iterations 20

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
use take_it_easy::neural::graph_transformer::GraphTransformerPolicyNet;
use take_it_easy::neural::model_io::{load_varstore, save_varstore};
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;
use take_it_easy::strategy::gt_boost::gt_beam_v1_select;

#[derive(Parser, Debug)]
#[command(name = "exit_trainer")]
struct Args {
    /// Number of ExIt iterations
    #[arg(long, default_value_t = 20)]
    iterations: usize,

    /// Expert games per iteration
    #[arg(long, default_value_t = 2000)]
    games_per_iter: usize,

    /// Evaluation games per iteration
    #[arg(long, default_value_t = 300)]
    eval_games: usize,

    /// Keep top X fraction of games (0.30 = top 30%)
    #[arg(long, default_value_t = 0.30)]
    top_percentile: f64,

    /// Training epochs per iteration
    #[arg(long, default_value_t = 15)]
    epochs_per_iter: usize,

    /// Batch size
    #[arg(long, default_value_t = 64)]
    batch_size: usize,

    /// Initial learning rate
    #[arg(long, default_value_t = 0.0003)]
    lr: f64,

    /// LR decay multiplier between iterations
    #[arg(long, default_value_t = 0.95)]
    lr_decay: f64,

    /// Beam width for V1Beam expert
    #[arg(long, default_value_t = 3)]
    beam_k: usize,

    /// Number of rollouts per beam candidate
    #[arg(long, default_value_t = 10)]
    beam_rollouts: usize,

    /// Line boost strength
    #[arg(long, default_value_t = 3.0)]
    line_boost: f64,

    /// V1 bonus strength
    #[arg(long, default_value_t = 2.0)]
    v1_bonus: f64,

    /// Early stop after N iterations without improvement
    #[arg(long, default_value_t = 3)]
    patience: usize,

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

    /// Path to initial model weights
    #[arg(long, default_value = "model_weights/graph_transformer_policy.safetensors")]
    load_path: String,

    /// Path to save best model
    #[arg(long, default_value = "model_weights/gt_exit_best.safetensors")]
    save_path: String,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

struct Sample {
    features: Tensor,
    target: i64,
    mask: Tensor,
    weight: f32,
}

/// Play games using V1Beam expert and collect training samples.
fn generate_expert_games(
    policy_net: &GraphTransformerPolicyNet,
    args: &Args,
    rng: &mut StdRng,
) -> (Vec<Sample>, Vec<i32>) {
    let mut all_samples = Vec::new();
    let mut game_scores = Vec::new();
    let start = Instant::now();

    for game_i in 0..args.games_per_iter {
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

            // Compute features for this state
            let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);

            // Build mask
            let mut mask_arr = [0.0f32; 19];
            for i in 0..19 {
                if plateau.tiles[i] != Tile(0, 0, 0) {
                    mask_arr[i] = f32::NEG_INFINITY;
                }
            }
            let mask_tensor = Tensor::from_slice(&mask_arr);

            // Expert chooses position using V1Beam
            let expert_pos = gt_beam_v1_select(
                &plateau,
                &tile,
                &deck,
                turn,
                policy_net,
                args.line_boost,
                args.beam_k,
                args.beam_rollouts,
                args.v1_bonus,
                rng,
            );

            game_samples.push((feat, expert_pos as i64, mask_tensor));

            // Play the expert's move
            plateau.tiles[expert_pos] = tile;
        }

        let score = result(&plateau);
        game_scores.push(score);

        // Store all samples with game score (filtering done later)
        for (feat, target, mask) in game_samples {
            all_samples.push(Sample {
                features: feat,
                target,
                mask,
                weight: score as f32, // placeholder, will be reweighted after filtering
            });
        }

        if (game_i + 1) % 200 == 0 || game_i == args.games_per_iter - 1 {
            let elapsed = start.elapsed().as_secs_f64();
            let recent_avg: f64 = game_scores
                .iter()
                .rev()
                .take(200)
                .map(|&s| s as f64)
                .sum::<f64>()
                / game_scores.len().min(200) as f64;
            print!(
                "\r  [{}/{}] samples={} avg={:.1} ({:.1}s)    ",
                game_i + 1,
                args.games_per_iter,
                all_samples.len(),
                recent_avg,
                elapsed
            );
        }
    }
    println!();

    (all_samples, game_scores)
}

/// Filter samples: keep only those from top-percentile games, reweight by score.
fn filter_top_percentile(
    samples: Vec<Sample>,
    scores: &[i32],
    games_per_iter: usize,
    top_percentile: f64,
) -> Vec<Sample> {
    // Compute score threshold at (1 - top_percentile) percentile
    let mut sorted_scores: Vec<i32> = scores.to_vec();
    sorted_scores.sort();
    let threshold_idx = ((1.0 - top_percentile) * sorted_scores.len() as f64) as usize;
    let threshold = sorted_scores[threshold_idx.min(sorted_scores.len() - 1)];

    // Figure out which game each sample belongs to and filter
    // Each game produces up to 19 samples. We track by score stored in weight field.
    let mut filtered = Vec::new();
    for s in samples {
        let game_score = s.weight as i32; // we stored the score as weight
        if game_score >= threshold {
            // Reweight: (score/100)^1.5 to favor high scores
            let w = (game_score as f64 / 100.0).powf(1.5) as f32;
            filtered.push(Sample {
                features: s.features,
                target: s.target,
                mask: s.mask,
                weight: w,
            });
        }
    }

    let kept_games = scores.iter().filter(|&&s| s >= threshold).count();
    println!(
        "  Threshold: {} pts | Kept: {}/{} games | Samples: {}",
        threshold, kept_games, games_per_iter, filtered.len()
    );

    filtered
}

/// Train one epoch of weighted cross-entropy. Returns average loss.
fn train_epoch(
    policy_net: &GraphTransformerPolicyNet,
    opt: &mut nn::Optimizer,
    samples: &[Sample],
    batch_size: usize,
    rng: &mut StdRng,
) -> f64 {
    let n = samples.len();
    let mut indices: Vec<usize> = (0..n).collect();
    indices.shuffle(rng);

    let n_batches = n / batch_size;
    if n_batches == 0 {
        return 0.0;
    }

    let mut total_loss = 0.0;

    for batch_i in 0..n_batches {
        let start = batch_i * batch_size;
        let end = start + batch_size;
        let batch_idx = &indices[start..end];

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
            (&per_sample_loss * &weight_tensor).sum(Kind::Float) / weight_tensor.sum(Kind::Float);

        opt.backward_step(&weighted_loss);
        total_loss += f64::try_from(&weighted_loss).unwrap();
    }

    total_loss / n_batches as f64
}

/// Evaluate model with GT Direct (argmax, no heuristics). Returns average score.
fn eval_model(
    policy_net: &GraphTransformerPolicyNet,
    n_games: usize,
    rng: &mut StdRng,
) -> f64 {
    let mut total = 0i64;

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

        total += result(&plateau) as i64;
    }

    total as f64 / n_games as f64
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║          Expert Iteration (ExIt) Trainer                    ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("Config:");
    println!("  Iterations:       {}", args.iterations);
    println!("  Games/iter:       {}", args.games_per_iter);
    println!("  Eval games:       {}", args.eval_games);
    println!("  Top percentile:   {:.0}%", args.top_percentile * 100.0);
    println!("  Epochs/iter:      {}", args.epochs_per_iter);
    println!("  LR:               {} (decay: {})", args.lr, args.lr_decay);
    println!("  Batch size:       {}", args.batch_size);
    println!("  Beam K:           {}", args.beam_k);
    println!("  Beam rollouts:    {}", args.beam_rollouts);
    println!("  Line boost:       {:.1}", args.line_boost);
    println!("  V1 bonus:         {:.1}", args.v1_bonus);
    println!("  Patience:         {}", args.patience);
    println!("  Load from:        {}", args.load_path);
    println!("  Save to:          {}", args.save_path);

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

    if !Path::new(&args.load_path).exists() {
        eprintln!("\nError: model weights not found: {}", args.load_path);
        return;
    }
    match load_varstore(&mut vs, &args.load_path) {
        Ok(()) => println!("\n  Loaded weights from {}", args.load_path),
        Err(e) => {
            eprintln!("\nError loading weights: {}", e);
            return;
        }
    }

    // Baseline evaluation
    println!("\n--- Baseline evaluation ({} games) ---", args.eval_games);
    let mut rng = StdRng::seed_from_u64(args.seed);
    let baseline = eval_model(&policy_net, args.eval_games, &mut rng);
    println!("  GT Direct baseline: {:.1} pts", baseline);

    let mut best_score = baseline;
    let mut no_improve = 0usize;
    let mut current_lr = args.lr;
    let total_start = Instant::now();

    // ═══════════════════════════════════════════
    //             ExIt Main Loop
    // ═══════════════════════════════════════════
    for iter in 0..args.iterations {
        let iter_start = Instant::now();
        println!(
            "\n══════════════════════════════════════════════════════════════"
        );
        println!(
            "  Iteration {}/{} | best={:.1} | lr={:.6} | no_improve={}/{}",
            iter + 1,
            args.iterations,
            best_score,
            current_lr,
            no_improve,
            args.patience
        );
        println!(
            "══════════════════════════════════════════════════════════════"
        );

        // 1. Generate expert data with V1Beam (uses current GT weights)
        println!(
            "\n  [1/4] Generating {} expert games (V1Beam k={} r={})...",
            args.games_per_iter, args.beam_k, args.beam_rollouts
        );
        let (samples, scores) = generate_expert_games(&policy_net, &args, &mut rng);

        let expert_avg: f64 = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
        println!("  Expert avg: {:.1} pts", expert_avg);

        // 2. Filter top percentile
        println!("\n  [2/4] Filtering top {:.0}%...", args.top_percentile * 100.0);
        let filtered = filter_top_percentile(
            samples,
            &scores,
            args.games_per_iter,
            args.top_percentile,
        );

        if filtered.len() < args.batch_size {
            println!("  Warning: too few samples ({}), skipping iteration", filtered.len());
            continue;
        }

        // 3. Train on filtered data
        println!(
            "\n  [3/4] Training {} epochs on {} samples (lr={:.6})...",
            args.epochs_per_iter,
            filtered.len(),
            current_lr
        );
        let mut opt = nn::Adam::default().build(&vs, current_lr).unwrap();

        for epoch in 0..args.epochs_per_iter {
            let loss = train_epoch(&policy_net, &mut opt, &filtered, args.batch_size, &mut rng);
            if epoch % 5 == 4 || epoch == args.epochs_per_iter - 1 {
                println!(
                    "    Epoch {:2}/{:2} | loss={:.4}",
                    epoch + 1,
                    args.epochs_per_iter,
                    loss
                );
            }
        }

        // 4. Evaluate
        println!(
            "\n  [4/4] Evaluating ({} games GT Direct)...",
            args.eval_games
        );
        let new_score = eval_model(&policy_net, args.eval_games, &mut rng);
        let delta = new_score - best_score;
        let iter_elapsed = iter_start.elapsed().as_secs_f64();

        if new_score > best_score {
            println!(
                "  Score: {:.1} pts ({:+.1} vs best) *** NEW BEST *** ({:.0}s)",
                new_score, delta, iter_elapsed
            );
            best_score = new_score;
            no_improve = 0;

            if let Err(e) = save_varstore(&vs, &args.save_path) {
                eprintln!("  Warning: failed to save: {}", e);
            } else {
                println!("  Saved to {}", args.save_path);
            }
        } else {
            no_improve += 1;
            println!(
                "  Score: {:.1} pts ({:+.1} vs best) | no_improve={}/{} ({:.0}s)",
                new_score, delta, no_improve, args.patience, iter_elapsed
            );

            if no_improve >= args.patience {
                println!("\n  Early stopping: no improvement for {} iterations", args.patience);
                break;
            }
        }

        // Decay LR
        current_lr *= args.lr_decay;
    }

    // ═══════════════════════════════════════════
    //            Final Summary
    // ═══════════════════════════════════════════
    let total_elapsed = total_start.elapsed().as_secs_f64();
    println!("\n══════════════════════════════════════════════════════════════");
    println!("                    ExIt COMPLETE");
    println!("══════════════════════════════════════════════════════════════");
    println!("\n  Baseline GT Direct:  {:.1} pts", baseline);
    println!("  Best ExIt score:     {:.1} pts", best_score);
    println!("  Delta:               {:+.1} pts", best_score - baseline);
    println!("  Total time:          {:.0}s ({:.1} min)", total_elapsed, total_elapsed / 60.0);

    // Final verification with fresh seed
    if best_score > baseline {
        println!("\n--- Final verification (500 games, fresh seed) ---");

        // Reload best weights
        if let Err(e) = load_varstore(&mut vs, &args.save_path) {
            eprintln!("  Warning: could not reload best weights: {}", e);
        }

        let mut verify_rng = StdRng::seed_from_u64(args.seed + 9999);
        let final_score = eval_model(&policy_net, 500, &mut verify_rng);
        println!("  GT Direct (best ExIt): {:.1} pts (500 games)", final_score);
    }
}
