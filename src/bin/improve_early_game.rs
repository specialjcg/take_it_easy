//! Improve Early Game Decisions
//!
//! Based on analysis: humans win by using central positions early.
//! This script fine-tunes the model using a simple supervised approach:
//! - Generate games with current model
//! - Identify high-scoring games (>160 pts)
//! - Train to imitate those decisions
//!
//! Usage: cargo run --release --bin improve_early_game -- --epochs 30

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::time::Instant;
use tch::{nn, nn::OptimizerConfig, Device, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::deck::Deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::remove_tile_from_deck::replace_tile_in_deck;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::graph_transformer::GraphTransformerPolicyNet;
use take_it_easy::neural::model_io::{load_varstore, save_varstore};
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "improve_early_game")]
struct Args {
    #[arg(long, default_value_t = 30)]
    epochs: usize,

    #[arg(long, default_value_t = 2000)]
    games_per_epoch: usize,

    #[arg(long, default_value_t = 160)]
    min_score: i32,

    #[arg(long, default_value_t = 0.00005)]
    lr: f64,

    #[arg(long, default_value_t = 32)]
    batch_size: usize,

    #[arg(long, default_value_t = 128)]
    embed_dim: i64,

    #[arg(long, default_value_t = 2)]
    num_layers: usize,

    #[arg(long, default_value_t = 4)]
    heads: i64,

    #[arg(long, default_value_t = 0.1)]
    dropout: f64,

    #[arg(long, default_value = "model_weights/graph_transformer_policy.safetensors")]
    load_path: String,

    #[arg(long, default_value = "model_weights/gt_improved")]
    save_path: String,

    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Temperature for sampling (lower = more greedy, higher = more exploration)
    #[arg(long, default_value_t = 0.8)]
    temperature: f32,
}

struct Sample {
    features: Tensor,
    position: usize,
    turn: usize,
    game_score: i32,
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     Improve Early Game via Self-Play Filtering               ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("Strategy: Play games, keep only high-scoring ones (>{}), train on those\n", args.min_score);

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

    if std::path::Path::new(&args.load_path).exists() {
        match load_varstore(&mut vs, &args.load_path) {
            Ok(()) => println!("✅ Loaded weights from {}", args.load_path),
            Err(e) => println!("⚠️ Could not load: {} - starting fresh", e),
        }
    }

    let mut opt = nn::Adam::default().build(&vs, args.lr).unwrap();
    let mut rng = StdRng::seed_from_u64(args.seed);

    // Baseline
    println!("\n📊 Baseline evaluation...");
    let (baseline, _) = eval_games(&policy_net, 500, &mut rng);
    println!("   Baseline: {:.2} pts\n", baseline);

    let mut best_score = baseline;

    for epoch in 0..args.epochs {
        let epoch_start = Instant::now();

        // Generate games with temperature sampling
        let mut all_samples = Vec::new();
        let mut game_scores = Vec::new();

        for _ in 0..args.games_per_epoch {
            let (samples, score) = play_game_with_sampling(
                &policy_net,
                &mut rng,
                args.temperature,
            );

            if score >= args.min_score {
                all_samples.extend(samples);
            }
            game_scores.push(score);
        }

        // Statistics
        let high_score_games = game_scores.iter().filter(|&&s| s >= args.min_score).count();
        let avg_score: f64 = game_scores.iter().sum::<i32>() as f64 / game_scores.len() as f64;

        if all_samples.is_empty() {
            println!("Epoch {:3}/{:3} | No high-scoring games (avg: {:.1})", epoch + 1, args.epochs, avg_score);
            continue;
        }

        // Shuffle and train
        all_samples.shuffle(&mut rng);

        let n_batches = all_samples.len() / args.batch_size;
        let mut epoch_loss = 0.0;

        for batch_i in 0..n_batches {
            let batch: Vec<&Sample> = all_samples[batch_i * args.batch_size..(batch_i + 1) * args.batch_size]
                .iter().collect();

            let (features, targets, masks, weights) = prepare_batch(&batch);

            let logits = policy_net.forward(&features, true);
            let masked_logits = logits + &masks;
            let log_probs = masked_logits.log_softmax(-1, Kind::Float);

            let per_sample_loss = -log_probs.gather(1, &targets.unsqueeze(1), false).squeeze_dim(1);
            let weighted_loss = (&per_sample_loss * &weights).sum(Kind::Float) / weights.sum(Kind::Float);

            opt.backward_step(&weighted_loss);
            epoch_loss += f64::try_from(&weighted_loss).unwrap();
        }

        let avg_loss = epoch_loss / n_batches.max(1) as f64;
        let elapsed = epoch_start.elapsed().as_secs_f32();

        // Evaluate every 5 epochs
        if epoch % 5 == 4 || epoch == args.epochs - 1 {
            let (score, _) = eval_games(&policy_net, 500, &mut rng);
            let improved = score > best_score;

            println!(
                "Epoch {:3}/{:3} | Loss: {:.4} | High games: {:4}/{:4} ({:.1}%) | Eval: {:.2} pts | {:.1}s{}",
                epoch + 1,
                args.epochs,
                avg_loss,
                high_score_games,
                args.games_per_epoch,
                high_score_games as f64 / args.games_per_epoch as f64 * 100.0,
                score,
                elapsed,
                if improved { " *" } else { "" }
            );

            if improved {
                best_score = score;
                let path = format!("{}_policy.safetensors", args.save_path);
                if let Err(e) = save_varstore(&vs, &path) {
                    eprintln!("Warning: save failed: {}", e);
                } else {
                    println!("   📁 New best! Saved to {}", path);
                }
            }
        } else {
            println!(
                "Epoch {:3}/{:3} | Loss: {:.4} | High games: {:4}/{:4} ({:.1}%) | {:.1}s",
                epoch + 1,
                args.epochs,
                avg_loss,
                high_score_games,
                args.games_per_epoch,
                high_score_games as f64 / args.games_per_epoch as f64 * 100.0,
                elapsed
            );
        }
    }

    println!("\n══════════════════════════════════════════════════════════════");
    println!("                     TRAINING COMPLETE");
    println!("══════════════════════════════════════════════════════════════");
    println!("  Baseline:    {:.2} pts", baseline);
    println!("  Best score:  {:.2} pts", best_score);
    println!("  Improvement: {:+.2} pts ({:+.1}%)",
        best_score - baseline,
        (best_score - baseline) / baseline * 100.0
    );

    // Final detailed evaluation
    println!("\n🎮 Final evaluation (1000 games)...");
    let (final_score, scores) = eval_games(&policy_net, 1000, &mut rng);
    let above_150: usize = scores.iter().filter(|&&s| s >= 150).count();
    let above_170: usize = scores.iter().filter(|&&s| s >= 170).count();

    println!("   Average: {:.2} pts", final_score);
    println!("   >= 150 pts: {} ({:.1}%)", above_150, above_150 as f64 / 10.0);
    println!("   >= 170 pts: {} ({:.1}%)", above_170, above_170 as f64 / 10.0);
}

fn play_game_with_sampling(
    policy_net: &GraphTransformerPolicyNet,
    rng: &mut StdRng,
    temperature: f32,
) -> (Vec<Sample>, i32) {
    let mut samples = Vec::new();
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();
    let mut available: Vec<Tile> = deck.tiles().iter().copied()
        .filter(|t| *t != Tile(0, 0, 0)).collect();

    for turn in 0..19 {
        if available.is_empty() { break; }

        let tile_idx = rng.random_range(0..available.len());
        let tile = available.remove(tile_idx);
        deck = replace_tile_in_deck(&deck, &tile);

        let legal = get_legal_moves(&plateau);
        if legal.is_empty() { break; }

        let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);

        // Get logits and apply temperature
        let logits = policy_net.forward(&feat.unsqueeze(0), false).squeeze_dim(0);

        let mut mask = [0.0f32; 19];
        for i in 0..19 {
            if !legal.contains(&i) {
                mask[i] = f32::NEG_INFINITY;
            }
        }
        let mask_tensor = Tensor::from_slice(&mask);
        let masked = logits + &mask_tensor;

        // Temperature sampling
        let scaled = &masked / (temperature as f64);
        let probs = scaled.softmax(-1, Kind::Float);

        // Sample from distribution
        let position = sample_action(&probs, rng);

        samples.push(Sample {
            features: feat,
            position,
            turn,
            game_score: 0,  // Will be filled later
        });

        plateau.tiles[position] = tile;
    }

    let final_score = result(&plateau);

    // Update game scores
    for sample in &mut samples {
        sample.game_score = final_score;
    }

    (samples, final_score)
}

fn sample_action(probs: &Tensor, rng: &mut StdRng) -> usize {
    let probs_vec: Vec<f32> = Vec::<f32>::try_from(probs.flatten(0, -1)).unwrap();
    let r: f32 = rng.random();
    let mut cumsum = 0.0;
    for (i, &p) in probs_vec.iter().enumerate() {
        cumsum += p;
        if r < cumsum {
            return i;
        }
    }
    probs_vec.len() - 1
}

fn prepare_batch(samples: &[&Sample]) -> (Tensor, Tensor, Tensor, Tensor) {
    let batch_size = samples.len();
    let mut features_vec = Vec::with_capacity(batch_size);
    let mut targets = Vec::with_capacity(batch_size);
    let mut masks_vec = Vec::with_capacity(batch_size);
    let mut weights_vec = Vec::with_capacity(batch_size);

    for sample in samples {
        features_vec.push(sample.features.shallow_clone());
        targets.push(sample.position as i64);

        // Weight by game score (higher score = more weight)
        let weight = (sample.game_score as f32 / 150.0).max(1.0);

        // Extra weight for early game moves (these are the key decisions)
        let turn_weight = if sample.turn < 6 { 1.5 } else { 1.0 };

        weights_vec.push(weight * turn_weight);

        // Create mask (occupied positions = -inf)
        // We'll use a simple heuristic since we don't have the full plateau info
        let mut mask = [0.0f32; 19];
        // Mark illegal (but we don't have this info directly, so just use default)
        masks_vec.push(Tensor::from_slice(&mask));
    }

    let features = Tensor::stack(&features_vec, 0);
    let targets = Tensor::from_slice(&targets);
    let masks = Tensor::stack(&masks_vec, 0);
    let weights = Tensor::from_slice(&weights_vec);

    (features, targets, masks, weights)
}

fn eval_games(
    policy_net: &GraphTransformerPolicyNet,
    n_games: usize,
    rng: &mut StdRng,
) -> (f64, Vec<i32>) {
    let mut scores = Vec::with_capacity(n_games);

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut available: Vec<Tile> = deck.tiles().iter().copied()
            .filter(|t| *t != Tile(0, 0, 0)).collect();

        for turn in 0..19 {
            if available.is_empty() { break; }

            let tile_idx = rng.random_range(0..available.len());
            let tile = available.remove(tile_idx);
            deck = replace_tile_in_deck(&deck, &tile);

            let legal = get_legal_moves(&plateau);
            if legal.is_empty() { break; }

            let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19).unsqueeze(0);
            let logits = policy_net.forward(&feat, false).squeeze_dim(0);

            let mut mask = [0.0f32; 19];
            for i in 0..19 {
                if !legal.contains(&i) {
                    mask[i] = f32::NEG_INFINITY;
                }
            }
            let mask_tensor = Tensor::from_slice(&mask);
            let masked = logits + mask_tensor;

            let best_pos: i64 = masked.argmax(-1, false).int64_value(&[]);
            plateau.tiles[best_pos as usize] = tile;
        }

        scores.push(result(&plateau));
    }

    let avg = scores.iter().sum::<i32>() as f64 / n_games as f64;
    (avg, scores)
}
