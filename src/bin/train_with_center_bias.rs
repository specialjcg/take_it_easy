//! Train Graph Transformer with Center Position Bias
//!
//! Key insight from analysis: Humans win more often when they prioritize
//! central positions (7, 8, 9, 10, 11) in early game.
//!
//! This training script adds positional value awareness to improve the model.
//!
//! Usage: cargo run --release --bin train_with_center_bias -- --epochs 50

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
use take_it_easy::scoring::scoring::result;

/// Position strategic values - center positions are more valuable
/// for early game flexibility
const POSITION_VALUES: [f32; 19] = [
    0.4, 0.5, 0.4,           // Col 0: edges (pos 0-2)
    0.5, 0.7, 0.7, 0.5,      // Col 1 (pos 3-6)
    0.6, 0.8, 1.0, 0.8, 0.6, // Col 2: center column (pos 7-11), 9 = center
    0.5, 0.7, 0.7, 0.5,      // Col 3 (pos 12-15)
    0.4, 0.5, 0.4,           // Col 4: edges (pos 16-18)
];

/// Line definitions for computing strategic features
const LINES: [(&[usize], usize); 15] = [
    // Horizontal (value 0 = tile.0)
    (&[0, 1, 2], 0),
    (&[3, 4, 5, 6], 0),
    (&[7, 8, 9, 10, 11], 0),
    (&[12, 13, 14, 15], 0),
    (&[16, 17, 18], 0),
    // Diagonal 1 (value 1 = tile.1)
    (&[0, 3, 7], 1),
    (&[1, 4, 8, 12], 1),
    (&[2, 5, 9, 13, 16], 1),
    (&[6, 10, 14, 17], 1),
    (&[11, 15, 18], 1),
    // Diagonal 2 (value 2 = tile.2)
    (&[2, 6, 11], 2),
    (&[1, 5, 10, 15], 2),
    (&[0, 4, 9, 14, 18], 2),
    (&[3, 8, 13, 17], 2),
    (&[7, 12, 16], 2),
];

#[derive(Parser, Debug)]
#[command(name = "train_with_center_bias")]
struct Args {
    #[arg(long, default_value_t = 50)]
    epochs: usize,

    #[arg(long, default_value_t = 1000)]
    games_per_epoch: usize,

    #[arg(long, default_value_t = 0.0001)]
    lr: f64,

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

    #[arg(long, default_value = "model_weights/gt_center_bias")]
    save_path: String,

    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Center bias strength (0.0 = no bias, 1.0 = strong bias)
    #[arg(long, default_value_t = 0.3)]
    center_bias: f32,

    /// Early game turns to apply center bias (0-5 typically)
    #[arg(long, default_value_t = 6)]
    early_game_turns: usize,
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     Train Graph Transformer with Center Bias                 ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("Config:");
    println!("  Center bias strength: {:.2}", args.center_bias);
    println!("  Early game turns:     {}", args.early_game_turns);
    println!("  Games per epoch:      {}", args.games_per_epoch);
    println!("  Epochs:               {}", args.epochs);

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

    // Load existing weights
    if std::path::Path::new(&args.load_path).exists() {
        match load_varstore(&mut vs, &args.load_path) {
            Ok(()) => println!("✅ Loaded weights from {}", args.load_path),
            Err(e) => println!("⚠️ Could not load: {} - starting fresh", e),
        }
    }

    let mut opt = nn::Adam::default().build(&vs, args.lr).unwrap();
    let mut rng = StdRng::seed_from_u64(args.seed);

    // Evaluate baseline
    println!("\n📊 Baseline evaluation...");
    let (baseline_score, _) = eval_games_detailed(&policy_net, 200, &mut rng);
    println!("   Baseline: {:.2} pts\n", baseline_score);

    let mut best_score = baseline_score;

    println!("🏋️ Training with center bias...\n");

    for epoch in 0..args.epochs {
        let epoch_start = Instant::now();
        let mut epoch_loss = 0.0;
        let mut epoch_samples = 0;

        // Generate self-play games with center bias reward shaping
        for _ in 0..args.games_per_epoch {
            let (samples, _score) = play_game_with_center_bias(
                &policy_net,
                &mut rng,
                args.center_bias,
                args.early_game_turns,
            );

            // Train on samples with reward-weighted loss
            for sample in &samples {
                let loss = train_step(
                    &policy_net,
                    &mut opt,
                    sample,
                );
                epoch_loss += loss;
                epoch_samples += 1;
            }
        }

        let avg_loss = epoch_loss / epoch_samples as f64;
        let elapsed = epoch_start.elapsed().as_secs_f32();

        // Evaluate every 5 epochs
        if epoch % 5 == 4 || epoch == args.epochs - 1 {
            let (score, _) = eval_games_detailed(&policy_net, 200, &mut rng);
            let improved = score > best_score;

            println!(
                "Epoch {:3}/{:3} | Loss: {:.4} | Score: {:.2} pts | {:.1}s{}",
                epoch + 1,
                args.epochs,
                avg_loss,
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
        } else if epoch % 1 == 0 {
            println!(
                "Epoch {:3}/{:3} | Loss: {:.4} | {:.1}s",
                epoch + 1,
                args.epochs,
                avg_loss,
                elapsed
            );
        }
    }

    println!("\n══════════════════════════════════════════════════════════════");
    println!("                     TRAINING COMPLETE");
    println!("══════════════════════════════════════════════════════════════");
    println!("  Baseline score:  {:.2} pts", baseline_score);
    println!("  Best score:      {:.2} pts", best_score);
    println!("  Improvement:     {:.2} pts ({:+.1}%)",
        best_score - baseline_score,
        (best_score - baseline_score) / baseline_score * 100.0
    );
}

struct TrainSample {
    features: Tensor,
    position: usize,
    reward: f32,  // Includes center bias reward shaping
    mask: Tensor,
}

fn play_game_with_center_bias(
    policy_net: &GraphTransformerPolicyNet,
    rng: &mut StdRng,
    center_bias: f32,
    early_turns: usize,
) -> (Vec<TrainSample>, i32) {
    let mut samples = Vec::new();
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();
    let mut available: Vec<Tile> = deck.tiles().iter().copied()
        .filter(|t| *t != Tile(0, 0, 0)).collect();

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

        // Get features
        let feat = convert_plateau_enhanced(&plateau, &tile, &deck, turn);

        // Get policy prediction
        let logits = policy_net.forward(&feat.unsqueeze(0), false).squeeze_dim(0);

        // Mask illegal moves
        let mut mask = [0.0f32; 19];
        for i in 0..19 {
            if !legal.contains(&i) {
                mask[i] = f32::NEG_INFINITY;
            }
        }
        let mask_tensor = Tensor::from_slice(&mask);
        let masked = &logits + &mask_tensor;

        // Add center bias for early game
        let biased = if turn < early_turns {
            let position_bonus: Vec<f32> = (0..19).map(|i| {
                if legal.contains(&i) {
                    POSITION_VALUES[i] * center_bias
                } else {
                    0.0
                }
            }).collect();
            let bonus_tensor = Tensor::from_slice(&position_bonus);
            masked + bonus_tensor
        } else {
            masked
        };

        // Sample action (with some exploration)
        let probs = biased.softmax(-1, Kind::Float);
        let position = sample_from_probs(&probs, rng);

        // Compute reward shaping
        let position_reward = if turn < early_turns {
            POSITION_VALUES[position] * center_bias
        } else {
            0.0
        };

        samples.push(TrainSample {
            features: feat,
            position,
            reward: position_reward,
            mask: mask_tensor,
        });

        plateau.tiles[position] = tile;
    }

    let final_score = result(&plateau);

    // Add final score to all samples (discounted)
    let n_samples = samples.len();
    for (i, sample) in samples.iter_mut().enumerate() {
        let discount = 0.99f32.powi((n_samples - 1 - i) as i32);
        sample.reward += (final_score as f32 / 200.0) * discount;
    }

    (samples, final_score)
}

fn sample_from_probs(probs: &Tensor, rng: &mut StdRng) -> usize {
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

fn train_step(
    policy_net: &GraphTransformerPolicyNet,
    opt: &mut nn::Optimizer,
    sample: &TrainSample,
) -> f64 {
    let logits = policy_net.forward(&sample.features.unsqueeze(0), true).squeeze_dim(0);
    let masked = logits + &sample.mask;
    let log_probs = masked.log_softmax(-1, Kind::Float);

    // Policy gradient loss: -log_prob * reward
    let log_prob = log_probs.double_value(&[sample.position as i64]);
    let loss = -log_prob * sample.reward as f64;

    let loss_tensor = Tensor::from_slice(&[loss as f32]);
    opt.backward_step(&loss_tensor);

    loss
}

fn convert_plateau_enhanced(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    turn: usize,
) -> Tensor {
    // Use the standard 47-channel conversion
    use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
    convert_plateau_for_gat_47ch(plateau, tile, deck, turn, 19)
}

fn eval_games_detailed(
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

            let feat = convert_plateau_enhanced(&plateau, &tile, &deck, turn).unsqueeze(0);
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
