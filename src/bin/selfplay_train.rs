//! AlphaZero Self-Play Training Loop for Take It Easy
//!
//! Subcommands:
//!   generate  - Self-play with current policy, save to CSV
//!   train     - Train policy + value net on self-play data
//!   benchmark - Compare two policies head-to-head
//!   loop      - Full iterative pipeline (generate → train → benchmark → repeat)
//!
//! Usage:
//!   CARGO_TARGET_DIR=target2 cargo build --release --bin selfplay_train
//!   ./target2/release/selfplay_train generate --model-path model_weights/graph_transformer_policy.safetensors --num-games 10000
//!   ./target2/release/selfplay_train train --data-dir selfplay_data --generations 0 --init-policy model_weights/graph_transformer_policy.safetensors
//!   ./target2/release/selfplay_train benchmark --model-a <gen0> --model-b <gen1> --num-games 500
//!   ./target2/release/selfplay_train loop --init-policy model_weights/graph_transformer_policy.safetensors --max-generations 10

use clap::{Parser, Subcommand};
use rand::prelude::*;
use rand::rngs::StdRng;
use std::error::Error;
use std::fs;
use std::io::Write as IoWrite;
use std::path::Path;
use std::time::Instant;
use tch::{nn, nn::OptimizerConfig, Device, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::deck::Deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::graph_transformer::{GraphTransformerPolicyNet, GraphTransformerValueNet};
use take_it_easy::neural::model_io::{load_varstore, save_varstore};
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;

// ============================================================
// CLI
// ============================================================

#[derive(Parser)]
#[command(name = "selfplay_train", about = "AlphaZero self-play training for Take It Easy")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate self-play games using current policy
    Generate {
        #[arg(long)]
        model_path: String,
        #[arg(long, default_value_t = 10000)]
        num_games: usize,
        #[arg(long, default_value_t = 1.0)]
        temperature: f64,
        #[arg(long, default_value_t = 8)]
        explore_turns: usize,
        #[arg(long, default_value = "selfplay_data")]
        data_dir: String,
        #[arg(long, default_value_t = 0)]
        generation: usize,
        #[arg(long)]
        seed: Option<u64>,
    },
    /// Train policy and value networks on self-play data
    Train {
        #[arg(long, default_value = "selfplay_data")]
        data_dir: String,
        #[arg(long, num_args = 1.., required = true)]
        generations: Vec<usize>,
        #[arg(long, num_args = 1..)]
        gen_weights: Option<Vec<f64>>,
        #[arg(long)]
        init_policy: Option<String>,
        #[arg(long)]
        init_value: Option<String>,
        #[arg(long, default_value_t = 50)]
        epochs: usize,
        #[arg(long, default_value_t = 128)]
        batch_size: usize,
        #[arg(long, default_value_t = 3e-4)]
        lr: f64,
        #[arg(long, default_value_t = 1e-4)]
        weight_decay: f64,
        #[arg(long, default_value = "model_weights")]
        output_dir: String,
        #[arg(long)]
        output_gen: Option<usize>,
        #[arg(long)]
        skip_value: bool,
        #[arg(long)]
        skip_policy: bool,
    },
    /// Compare two policies head-to-head on identical tile sequences
    Benchmark {
        #[arg(long)]
        model_a: String,
        #[arg(long)]
        model_b: String,
        #[arg(long, default_value_t = 500)]
        num_games: usize,
        #[arg(long)]
        seed: Option<u64>,
    },
    /// Full iterative self-play loop (generate + train + benchmark, repeat)
    Loop {
        #[arg(long)]
        init_policy: String,
        #[arg(long)]
        init_value: Option<String>,
        #[arg(long, default_value_t = 10)]
        max_generations: usize,
        #[arg(long, default_value_t = 10000)]
        games_per_gen: usize,
        #[arg(long, default_value_t = 1.0)]
        temperature: f64,
        #[arg(long, default_value_t = 8)]
        explore_turns: usize,
        #[arg(long, default_value_t = 50)]
        epochs: usize,
        #[arg(long, default_value_t = 128)]
        batch_size: usize,
        #[arg(long, default_value_t = 3e-4)]
        lr: f64,
        #[arg(long, default_value_t = 1e-4)]
        weight_decay: f64,
        #[arg(long, default_value_t = 500)]
        benchmark_games: usize,
        #[arg(long, default_value_t = 1.0)]
        accept_threshold: f64,
        #[arg(long, default_value = "selfplay_data")]
        data_dir: String,
        #[arg(long, default_value = "model_weights")]
        output_dir: String,
        #[arg(long)]
        seed: Option<u64>,
    },
}

// ============================================================
// Data types
// ============================================================

struct TurnRecord {
    game_idx: usize,
    turn: usize,
    plateau: [i32; 19],        // encoded: 0=empty, v1*100+v2*10+v3
    tile: (i32, i32, i32),     // current tile values
    chosen_position: usize,
    final_score: i32,
}

// ============================================================
// Helpers
// ============================================================

fn encode_tile(t: &Tile) -> i32 {
    if *t == Tile(0, 0, 0) { 0 } else { t.0 * 100 + t.1 * 10 + t.2 }
}

fn decode_tile(v: i32) -> Tile {
    if v == 0 { Tile(0, 0, 0) } else { Tile(v / 100, (v / 10) % 10, v % 10) }
}

fn reconstruct_deck_from_plateau(plateau: &Plateau) -> Deck {
    let mut deck = create_deck();
    for tile in &plateau.tiles {
        if *tile != Tile(0, 0, 0) {
            deck = replace_tile_in_deck(&deck, tile);
        }
    }
    deck
}

fn cosine_lr(base_lr: f64, epoch: usize, total_epochs: usize, min_ratio: f64) -> f64 {
    let min_lr = base_lr * min_ratio;
    let progress = epoch as f64 / total_epochs as f64;
    min_lr + 0.5 * (base_lr - min_lr) * (1.0 + (std::f64::consts::PI * progress).cos())
}

fn generate_tile_sequence(rng: &mut StdRng) -> Vec<Tile> {
    let mut deck = create_deck();
    let mut tiles = Vec::with_capacity(19);
    for _ in 0..19 {
        let available = get_available_tiles(&deck);
        if available.is_empty() { break; }
        let tile = *available.choose(rng).unwrap();
        tiles.push(tile);
        deck = replace_tile_in_deck(&deck, &tile);
    }
    tiles
}

fn compute_masked_logits(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    turn: usize,
    policy_net: &GraphTransformerPolicyNet,
) -> Tensor {
    let feat = convert_plateau_for_gat_47ch(plateau, tile, deck, turn, 19).unsqueeze(0);
    let logits = policy_net.forward(&feat, false).squeeze_dim(0);
    let mut mask = [0.0f32; 19];
    for i in 0..19 {
        if plateau.tiles[i] != Tile(0, 0, 0) {
            mask[i] = f32::NEG_INFINITY;
        }
    }
    logits + Tensor::from_slice(&mask)
}

fn load_policy(path: &str) -> Result<(nn::VarStore, GraphTransformerPolicyNet), Box<dyn Error>> {
    let mut vs = nn::VarStore::new(Device::Cpu);
    let net = GraphTransformerPolicyNet::new(&vs, 47, 128, 2, 4, 0.1);
    load_varstore(&mut vs, path)?;
    Ok((vs, net))
}

fn decode_sample_state(sample: &TurnRecord) -> (Plateau, Tile) {
    let mut plateau = create_plateau_empty();
    for i in 0..19 {
        plateau.tiles[i] = decode_tile(sample.plateau[i]);
    }
    let tile = Tile(sample.tile.0, sample.tile.1, sample.tile.2);
    (plateau, tile)
}

fn compute_sample_weights(samples: &[TurnRecord], gen_weight: f64) -> Vec<f32> {
    let scores: Vec<f64> = samples.iter().map(|s| s.final_score as f64).collect();
    let mean = scores.iter().sum::<f64>() / scores.len() as f64;
    let std = (scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / scores.len() as f64)
        .sqrt()
        .max(1.0);

    scores
        .iter()
        .map(|&s| {
            let advantage = (s - mean) / std;
            let w = (1.0 + advantage.exp()).ln(); // softplus
            (w * gen_weight) as f32
        })
        .collect()
}

fn score_stats(scores: &[i32]) -> (f64, i32, i32, i32) {
    let avg = scores.iter().map(|&s| s as f64).sum::<f64>() / scores.len() as f64;
    let mut sorted = scores.to_vec();
    sorted.sort();
    let median = sorted[sorted.len() / 2];
    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    (avg, median, min, max)
}

// ============================================================
// CSV I/O
// ============================================================

fn save_csv(records: &[TurnRecord], path: &str) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    let mut wtr = csv::Writer::from_path(path)?;
    let mut header: Vec<String> = vec!["game_idx".into(), "turn".into()];
    for i in 0..19 {
        header.push(format!("p{}", i));
    }
    header.extend(["t0", "t1", "t2", "chosen", "score"].iter().map(|s| s.to_string()));
    wtr.write_record(&header)?;

    for r in records {
        let mut row: Vec<String> = vec![r.game_idx.to_string(), r.turn.to_string()];
        for i in 0..19 {
            row.push(r.plateau[i].to_string());
        }
        row.push(r.tile.0.to_string());
        row.push(r.tile.1.to_string());
        row.push(r.tile.2.to_string());
        row.push(r.chosen_position.to_string());
        row.push(r.final_score.to_string());
        wtr.write_record(&row)?;
    }
    wtr.flush()?;
    Ok(())
}

fn load_csv(path: &str) -> Result<Vec<TurnRecord>, Box<dyn Error>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let mut records = Vec::new();
    for row in rdr.records() {
        let row = row?;
        let game_idx: usize = row[0].parse()?;
        let turn: usize = row[1].parse()?;
        let mut plateau = [0i32; 19];
        for i in 0..19 {
            plateau[i] = row[2 + i].parse()?;
        }
        let tile = (row[21].parse()?, row[22].parse()?, row[23].parse()?);
        let chosen: usize = row[24].parse()?;
        let score: i32 = row[25].parse()?;
        records.push(TurnRecord {
            game_idx,
            turn,
            plateau,
            tile,
            chosen_position: chosen,
            final_score: score,
        });
    }
    Ok(records)
}

// ============================================================
// Self-play generation
// ============================================================

fn run_generate(
    model_path: &str,
    num_games: usize,
    temperature: f64,
    explore_turns: usize,
    save_path: &str,
    seed: u64,
) -> Result<Vec<i32>, Box<dyn Error>> {
    println!("Generating {} self-play games (temp={}, explore_turns={})", num_games, temperature, explore_turns);
    println!("Policy: {}", model_path);

    let (_vs, policy_net) = load_policy(model_path)?;
    let _guard = tch::no_grad_guard();

    let mut rng = StdRng::seed_from_u64(seed);
    let mut all_records = Vec::with_capacity(num_games * 19);
    let mut scores = Vec::with_capacity(num_games);
    let start = Instant::now();

    for game_idx in 0..num_games {
        let tiles = generate_tile_sequence(&mut rng);
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut game_turns: Vec<(usize, [i32; 19], (i32, i32, i32), usize)> = Vec::with_capacity(19);

        for (turn, tile) in tiles.iter().enumerate() {
            let legal = get_legal_moves(&plateau);
            if legal.is_empty() {
                break;
            }

            let ml = compute_masked_logits(&plateau, tile, &deck, turn, &policy_net);

            let pos = if turn < explore_turns && temperature > 1e-6 {
                let probs = (&ml / temperature).softmax(-1, Kind::Float);
                probs.multinomial(1, true).int64_value(&[0]) as usize
            } else {
                ml.argmax(-1, false).int64_value(&[]) as usize
            };

            // Record state BEFORE placing tile
            let mut encoded = [0i32; 19];
            for i in 0..19 {
                encoded[i] = encode_tile(&plateau.tiles[i]);
            }
            game_turns.push((turn, encoded, (tile.0, tile.1, tile.2), pos));

            plateau.tiles[pos] = *tile;
            deck = replace_tile_in_deck(&deck, tile);
        }

        let score = result(&plateau);
        scores.push(score);

        for (turn, encoded, tile, chosen) in game_turns {
            all_records.push(TurnRecord {
                game_idx,
                turn,
                plateau: encoded,
                tile,
                chosen_position: chosen,
                final_score: score,
            });
        }

        if (game_idx + 1) % 1000 == 0 {
            let (avg, _, _, _) = score_stats(&scores);
            println!(
                "  {} games ({:.1}s) - avg score: {:.1}",
                game_idx + 1,
                start.elapsed().as_secs_f64(),
                avg
            );
        }
    }

    let (avg, median, min, max) = score_stats(&scores);
    println!(
        "Done: {} games, avg={:.1}, median={}, min={}, max={}, time={:.1}s",
        num_games,
        avg,
        median,
        min,
        max,
        start.elapsed().as_secs_f64()
    );
    println!("Records: {}", all_records.len());

    save_csv(&all_records, save_path)?;
    println!("Saved to {}", save_path);

    Ok(scores)
}

// ============================================================
// Training: Policy
// ============================================================

fn prepare_policy_batch(
    samples: &[TurnRecord],
    indices: &[usize],
    weights: &[f32],
) -> (Tensor, Tensor, Tensor, Tensor) {
    let mut features = Vec::with_capacity(indices.len());
    let mut masks = Vec::with_capacity(indices.len());
    let mut targets = Vec::with_capacity(indices.len());
    let mut w = Vec::with_capacity(indices.len());

    for &idx in indices {
        let s = &samples[idx];
        let (plateau, tile) = decode_sample_state(s);
        let deck = reconstruct_deck_from_plateau(&plateau);
        features.push(convert_plateau_for_gat_47ch(&plateau, &tile, &deck, s.turn, 19));

        let mut mask = [0.0f32; 19];
        for i in 0..19 {
            if plateau.tiles[i] != Tile(0, 0, 0) {
                mask[i] = f32::NEG_INFINITY;
            }
        }
        masks.push(Tensor::from_slice(&mask));
        targets.push(s.chosen_position as i64);
        w.push(weights[idx]);
    }

    (
        Tensor::stack(&features, 0),                    // [B, 19, 47]
        Tensor::stack(&masks, 0),                       // [B, 19]
        Tensor::from_slice(&targets).unsqueeze(1),      // [B, 1]
        Tensor::from_slice(&w),                         // [B]
    )
}

fn train_policy(
    samples: &[TurnRecord],
    sample_weights: &[f32],
    init_path: Option<&str>,
    save_path: &str,
    epochs: usize,
    batch_size: usize,
    lr: f64,
    weight_decay: f64,
) -> Result<f64, Box<dyn Error>> {
    println!("\n=== Training Policy ({} samples) ===", samples.len());

    let mut vs = nn::VarStore::new(Device::Cpu);
    let policy_net = GraphTransformerPolicyNet::new(&vs, 47, 128, 2, 4, 0.1);

    if let Some(path) = init_path {
        println!("Init weights: {}", path);
        load_varstore(&mut vs, path)?;
    }

    let mut opt = nn::Adam {
        wd: weight_decay,
        ..Default::default()
    }
    .build(&vs, lr)?;

    // Train/val split (90/10)
    let n = samples.len();
    let n_val = (n / 10).max(1);
    let n_train = n - n_val;
    let mut rng = StdRng::seed_from_u64(42);
    let mut indices: Vec<usize> = (0..n).collect();
    indices.shuffle(&mut rng);
    let train_idx: Vec<usize> = indices[..n_train].to_vec();
    let val_idx: Vec<usize> = indices[n_train..].to_vec();

    let mut best_val_loss = f64::INFINITY;
    let start = Instant::now();

    for epoch in 0..epochs {
        let clr = cosine_lr(lr, epoch, epochs, 0.01);
        opt.set_lr(clr);

        let mut perm = train_idx.clone();
        perm.shuffle(&mut rng);

        let mut epoch_loss = 0.0;
        let mut num_batches = 0;

        for batch_start in (0..perm.len()).step_by(batch_size) {
            let batch_end = (batch_start + batch_size).min(perm.len());
            let batch = &perm[batch_start..batch_end];
            let (feat, mask, targets, weights) =
                prepare_policy_batch(samples, batch, sample_weights);

            let logits = policy_net.forward(&feat, true);
            let masked = logits + &mask;
            let log_probs = masked.log_softmax(-1, Kind::Float);
            let chosen = log_probs.gather(1, &targets, false).squeeze_dim(1);
            let loss = -(&weights * &chosen).mean(Kind::Float);

            opt.backward_step(&loss);
            epoch_loss += f64::try_from(&loss).unwrap();
            num_batches += 1;
        }

        // Validation
        let val_loss = tch::no_grad(|| {
            let mut total = 0.0;
            let mut count = 0;
            for batch_start in (0..val_idx.len()).step_by(batch_size) {
                let batch_end = (batch_start + batch_size).min(val_idx.len());
                let batch = &val_idx[batch_start..batch_end];
                let (feat, mask, targets, weights) =
                    prepare_policy_batch(samples, batch, sample_weights);
                let logits = policy_net.forward(&feat, false);
                let masked = logits + &mask;
                let log_probs = masked.log_softmax(-1, Kind::Float);
                let chosen = log_probs.gather(1, &targets, false).squeeze_dim(1);
                let loss = -(&weights * &chosen).mean(Kind::Float);
                total += f64::try_from(&loss).unwrap();
                count += 1;
            }
            if count > 0 { total / count as f64 } else { 0.0 }
        });

        if val_loss < best_val_loss {
            best_val_loss = val_loss;
            if let Err(e) = save_varstore(&vs, save_path) {
                eprintln!("Warning: failed to save policy: {}", e);
            }
        }

        if epoch % 10 == 0 || epoch == epochs - 1 {
            println!(
                "  Epoch {}/{}: train={:.4}, val={:.4}, lr={:.2e} ({:.0}s)",
                epoch + 1,
                epochs,
                epoch_loss / num_batches.max(1) as f64,
                val_loss,
                clr,
                start.elapsed().as_secs_f64()
            );
        }
    }

    println!("Policy best val loss: {:.4}, saved to {}", best_val_loss, save_path);
    Ok(best_val_loss)
}

// ============================================================
// Training: Value
// ============================================================

fn prepare_value_batch(
    samples: &[TurnRecord],
    indices: &[usize],
    score_mean: f64,
    score_std: f64,
) -> (Tensor, Tensor) {
    let mut features = Vec::with_capacity(indices.len());
    let mut targets = Vec::with_capacity(indices.len());

    for &idx in indices {
        let s = &samples[idx];
        let (plateau, tile) = decode_sample_state(s);
        let deck = reconstruct_deck_from_plateau(&plateau);
        features.push(convert_plateau_for_gat_47ch(&plateau, &tile, &deck, s.turn, 19));

        let norm = ((s.final_score as f64 - score_mean) / score_std).clamp(-1.0, 1.0) as f32;
        targets.push(norm);
    }

    (
        Tensor::stack(&features, 0),                     // [B, 19, 47]
        Tensor::from_slice(&targets).unsqueeze(1),       // [B, 1]
    )
}

fn train_value(
    samples: &[TurnRecord],
    init_path: Option<&str>,
    save_path: &str,
    epochs: usize,
    batch_size: usize,
    lr: f64,
    weight_decay: f64,
) -> Result<f64, Box<dyn Error>> {
    println!("\n=== Training Value ({} samples) ===", samples.len());

    let mut vs = nn::VarStore::new(Device::Cpu);
    let value_net = GraphTransformerValueNet::new(&vs, 47, 128, 2, 4, 0.1);

    if let Some(path) = init_path {
        println!("Init weights: {}", path);
        load_varstore(&mut vs, path)?;
    }

    let mut opt = nn::Adam {
        wd: weight_decay,
        ..Default::default()
    }
    .build(&vs, lr)?;

    let score_mean = 140.0;
    let score_std = 40.0;

    // Log actual score distribution
    let real_scores: Vec<f64> = samples.iter().map(|s| s.final_score as f64).collect();
    let real_mean = real_scores.iter().sum::<f64>() / real_scores.len() as f64;
    let real_std = (real_scores
        .iter()
        .map(|s| (s - real_mean).powi(2))
        .sum::<f64>()
        / real_scores.len() as f64)
        .sqrt();
    println!(
        "Score stats: actual mean={:.1}, std={:.1} | norm mean={}, std={}",
        real_mean, real_std, score_mean, score_std
    );

    // Train/val split (90/10)
    let n = samples.len();
    let n_val = (n / 10).max(1);
    let n_train = n - n_val;
    let mut rng = StdRng::seed_from_u64(42);
    let mut indices: Vec<usize> = (0..n).collect();
    indices.shuffle(&mut rng);
    let train_idx: Vec<usize> = indices[..n_train].to_vec();
    let val_idx: Vec<usize> = indices[n_train..].to_vec();

    let mut best_val_loss = f64::INFINITY;
    let start = Instant::now();

    for epoch in 0..epochs {
        let clr = cosine_lr(lr, epoch, epochs, 0.01);
        opt.set_lr(clr);

        let mut perm = train_idx.clone();
        perm.shuffle(&mut rng);

        let mut epoch_loss = 0.0;
        let mut num_batches = 0;

        for batch_start in (0..perm.len()).step_by(batch_size) {
            let batch_end = (batch_start + batch_size).min(perm.len());
            let batch = &perm[batch_start..batch_end];
            let (feat, targets) = prepare_value_batch(samples, batch, score_mean, score_std);
            let pred = value_net.forward(&feat, true);
            let loss = pred.mse_loss(&targets, tch::Reduction::Mean);
            opt.backward_step(&loss);
            epoch_loss += f64::try_from(&loss).unwrap();
            num_batches += 1;
        }

        // Validation
        let val_loss = tch::no_grad(|| {
            let mut total = 0.0;
            let mut count = 0;
            for batch_start in (0..val_idx.len()).step_by(batch_size) {
                let batch_end = (batch_start + batch_size).min(val_idx.len());
                let batch = &val_idx[batch_start..batch_end];
                let (feat, targets) = prepare_value_batch(samples, batch, score_mean, score_std);
                let pred = value_net.forward(&feat, false);
                let loss = pred.mse_loss(&targets, tch::Reduction::Mean);
                total += f64::try_from(&loss).unwrap();
                count += 1;
            }
            if count > 0 { total / count as f64 } else { 0.0 }
        });

        if val_loss < best_val_loss {
            best_val_loss = val_loss;
            if let Err(e) = save_varstore(&vs, save_path) {
                eprintln!("Warning: failed to save value net: {}", e);
            }
        }

        if epoch % 10 == 0 || epoch == epochs - 1 {
            println!(
                "  Epoch {}/{}: train={:.4}, val={:.4}, lr={:.2e} ({:.0}s)",
                epoch + 1,
                epochs,
                epoch_loss / num_batches.max(1) as f64,
                val_loss,
                clr,
                start.elapsed().as_secs_f64()
            );
        }
    }

    println!("Value best val loss: {:.4}, saved to {}", best_val_loss, save_path);
    Ok(best_val_loss)
}

// ============================================================
// Benchmark
// ============================================================

fn play_game_with_policy(tiles: &[Tile], policy_net: &GraphTransformerPolicyNet) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }
        let ml = compute_masked_logits(&plateau, tile, &deck, turn, policy_net);
        let pos = ml.argmax(-1, false).int64_value(&[]) as usize;
        plateau.tiles[pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }
    result(&plateau)
}

fn run_benchmark(
    model_a_path: &str,
    model_b_path: &str,
    num_games: usize,
    seed: u64,
) -> Result<(f64, f64), Box<dyn Error>> {
    println!("\n=== Benchmark ({} games) ===", num_games);
    println!("  A: {}", model_a_path);
    println!("  B: {}", model_b_path);

    let (_vs_a, policy_a) = load_policy(model_a_path)?;
    let (_vs_b, policy_b) = load_policy(model_b_path)?;
    let _guard = tch::no_grad_guard();

    let mut rng = StdRng::seed_from_u64(seed);
    let mut scores_a = Vec::with_capacity(num_games);
    let mut scores_b = Vec::with_capacity(num_games);
    let start = Instant::now();

    for i in 0..num_games {
        let tiles = generate_tile_sequence(&mut rng);
        let sa = play_game_with_policy(&tiles, &policy_a);
        let sb = play_game_with_policy(&tiles, &policy_b);
        scores_a.push(sa);
        scores_b.push(sb);

        if (i + 1) % 100 == 0 {
            let avg_a: f64 = scores_a.iter().map(|&s| s as f64).sum::<f64>() / scores_a.len() as f64;
            let avg_b: f64 = scores_b.iter().map(|&s| s as f64).sum::<f64>() / scores_b.len() as f64;
            println!("  {} games: A={:.1}, B={:.1}", i + 1, avg_a, avg_b);
        }
    }

    let (avg_a, median_a, _, _) = score_stats(&scores_a);
    let (avg_b, median_b, _, _) = score_stats(&scores_b);

    let wins_a = scores_a.iter().zip(&scores_b).filter(|(&a, &b)| a > b).count();
    let wins_b = scores_a.iter().zip(&scores_b).filter(|(&a, &b)| b > a).count();
    let draws = num_games - wins_a - wins_b;

    println!("\nResults ({:.1}s):", start.elapsed().as_secs_f64());
    println!("  Model A: avg={:.1}, median={}", avg_a, median_a);
    println!("  Model B: avg={:.1}, median={}", avg_b, median_b);
    println!(
        "  Wins: A={} ({:.1}%), B={} ({:.1}%), draws={}",
        wins_a,
        wins_a as f64 / num_games as f64 * 100.0,
        wins_b,
        wins_b as f64 / num_games as f64 * 100.0,
        draws
    );

    Ok((avg_a, avg_b))
}

// ============================================================
// Loop
// ============================================================

fn run_loop(
    init_policy: &str,
    init_value: Option<&str>,
    max_generations: usize,
    games_per_gen: usize,
    temperature: f64,
    explore_turns: usize,
    epochs: usize,
    batch_size: usize,
    lr: f64,
    weight_decay: f64,
    benchmark_games: usize,
    accept_threshold: f64,
    data_dir: &str,
    output_dir: &str,
    seed: u64,
) -> Result<(), Box<dyn Error>> {
    println!("{}", "=".repeat(60));
    println!("AlphaZero Self-Play Loop");
    println!("{}", "=".repeat(60));
    println!("Max generations: {}, Games/gen: {}", max_generations, games_per_gen);
    println!("Temperature: {}, Explore turns: {}", temperature, explore_turns);
    println!("Epochs: {}, Batch size: {}, LR: {}", epochs, batch_size, lr);
    println!("Benchmark games: {}, Accept threshold: {}", benchmark_games, accept_threshold);

    fs::create_dir_all(data_dir)?;
    fs::create_dir_all(output_dir)?;

    let mut current_policy = init_policy.to_string();
    let mut current_value: Option<String> = init_value.map(|s| s.to_string());

    // History log
    let history_path = format!("{}/selfplay_history.csv", data_dir);
    let write_header = !Path::new(&history_path).exists();
    if write_header {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&history_path)?;
        writeln!(
            f,
            "generation,num_games,avg_score,policy_val_loss,value_val_loss,bench_avg_old,bench_avg_new,accepted"
        )?;
    }

    let loop_start = Instant::now();
    let gen_weight_schedule = [1.0, 0.5, 0.25];

    for gen in 0..max_generations {
        println!("\n{}", "=".repeat(60));
        println!("Generation {} / {}", gen + 1, max_generations);
        println!("{}", "=".repeat(60));

        // 1. Generate self-play data
        let data_path = format!("{}/gen{}.csv", data_dir, gen);
        let gen_seed = seed.wrapping_add(gen as u64 * 1_000_000);
        let scores = run_generate(
            &current_policy,
            games_per_gen,
            temperature,
            explore_turns,
            &data_path,
            gen_seed,
        )?;
        let avg_score: f64 = scores.iter().map(|&s| s as f64).sum::<f64>() / scores.len() as f64;

        // 2. Load training data (sliding window of last 3 generations)
        let mut all_samples: Vec<TurnRecord> = Vec::new();
        let mut all_weights: Vec<f32> = Vec::new();

        for (offset, &gw) in gen_weight_schedule.iter().enumerate() {
            if gen < offset {
                break;
            }
            let g = gen - offset;
            let path = format!("{}/gen{}.csv", data_dir, g);
            if !Path::new(&path).exists() {
                break;
            }
            let samples = load_csv(&path)?;
            let weights = compute_sample_weights(&samples, gw);
            let count = samples.len();
            all_samples.extend(samples);
            all_weights.extend(weights);
            println!("  Loaded gen {} ({} samples, weight={:.2})", g, count, gw);
        }
        println!("  Total training samples: {}", all_samples.len());

        // 3. Train policy
        let new_policy_path = format!(
            "{}/gt_selfplay_gen{}_policy.safetensors",
            output_dir,
            gen + 1
        );
        let policy_loss = train_policy(
            &all_samples,
            &all_weights,
            Some(&current_policy),
            &new_policy_path,
            epochs,
            batch_size,
            lr,
            weight_decay,
        )?;

        // 4. Train value
        let new_value_path = format!(
            "{}/gt_selfplay_gen{}_value.safetensors",
            output_dir,
            gen + 1
        );
        let value_loss = train_value(
            &all_samples,
            current_value.as_deref(),
            &new_value_path,
            epochs,
            batch_size,
            lr,
            weight_decay,
        )?;

        // 5. Benchmark new vs current
        let bench_seed = seed.wrapping_add(999_999 + gen as u64 * 1_000);
        let (avg_old, avg_new) =
            run_benchmark(&current_policy, &new_policy_path, benchmark_games, bench_seed)?;

        // 6. Accept/reject
        let accepted = avg_new > avg_old + accept_threshold;
        if accepted {
            println!(
                "\n>>> ACCEPTED gen {} ({:.1} > {:.1} + {:.1})",
                gen + 1,
                avg_new,
                avg_old,
                accept_threshold
            );
            current_policy = new_policy_path;
            current_value = Some(new_value_path);
        } else {
            println!(
                "\n>>> REJECTED gen {} ({:.1} <= {:.1} + {:.1})",
                gen + 1,
                avg_new,
                avg_old,
                accept_threshold
            );
        }

        // 7. Log to history
        {
            let mut f = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&history_path)?;
            writeln!(
                f,
                "{},{},{:.1},{:.4},{:.4},{:.1},{:.1},{}",
                gen + 1,
                games_per_gen,
                avg_score,
                policy_loss,
                value_loss,
                avg_old,
                avg_new,
                accepted
            )?;
        }

        println!(
            "Elapsed: {:.0}s total",
            loop_start.elapsed().as_secs_f64()
        );
    }

    println!("\n{}", "=".repeat(60));
    println!("Loop complete!");
    println!("Final policy: {}", current_policy);
    if let Some(ref v) = current_value {
        println!("Final value:  {}", v);
    }
    println!(
        "Total time: {:.0}s",
        loop_start.elapsed().as_secs_f64()
    );

    Ok(())
}

// ============================================================
// Main
// ============================================================

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate {
            model_path,
            num_games,
            temperature,
            explore_turns,
            data_dir,
            generation,
            seed,
        } => {
            let save_path = format!("{}/gen{}.csv", data_dir, generation);
            run_generate(
                &model_path,
                num_games,
                temperature,
                explore_turns,
                &save_path,
                seed.unwrap_or(42),
            )?;
        }

        Commands::Train {
            data_dir,
            generations,
            gen_weights,
            init_policy,
            init_value,
            epochs,
            batch_size,
            lr,
            weight_decay,
            output_dir,
            output_gen,
            skip_value,
            skip_policy,
        } => {
            let output_gen = output_gen.unwrap_or_else(|| generations.iter().max().unwrap() + 1);
            let gw = gen_weights.unwrap_or_else(|| vec![1.0; generations.len()]);
            assert_eq!(
                gw.len(),
                generations.len(),
                "--gen-weights length must match --generations length"
            );

            let mut all_samples = Vec::new();
            let mut all_weights = Vec::new();

            for (i, &gen) in generations.iter().enumerate() {
                let path = format!("{}/gen{}.csv", data_dir, gen);
                println!("Loading {} (weight={:.2})", path, gw[i]);
                let samples = load_csv(&path)?;
                let weights = compute_sample_weights(&samples, gw[i]);
                all_samples.extend(samples);
                all_weights.extend(weights);
            }
            println!("Total samples: {}", all_samples.len());

            fs::create_dir_all(&output_dir)?;

            if !skip_policy {
                let save = format!(
                    "{}/gt_selfplay_gen{}_policy.safetensors",
                    output_dir, output_gen
                );
                train_policy(
                    &all_samples,
                    &all_weights,
                    init_policy.as_deref(),
                    &save,
                    epochs,
                    batch_size,
                    lr,
                    weight_decay,
                )?;
            }

            if !skip_value {
                let save = format!(
                    "{}/gt_selfplay_gen{}_value.safetensors",
                    output_dir, output_gen
                );
                train_value(
                    &all_samples,
                    init_value.as_deref(),
                    &save,
                    epochs,
                    batch_size,
                    lr,
                    weight_decay,
                )?;
            }
        }

        Commands::Benchmark {
            model_a,
            model_b,
            num_games,
            seed,
        } => {
            run_benchmark(&model_a, &model_b, num_games, seed.unwrap_or(42))?;
        }

        Commands::Loop {
            init_policy,
            init_value,
            max_generations,
            games_per_gen,
            temperature,
            explore_turns,
            epochs,
            batch_size,
            lr,
            weight_decay,
            benchmark_games,
            accept_threshold,
            data_dir,
            output_dir,
            seed,
        } => {
            run_loop(
                &init_policy,
                init_value.as_deref(),
                max_generations,
                games_per_gen,
                temperature,
                explore_turns,
                epochs,
                batch_size,
                lr,
                weight_decay,
                benchmark_games,
                accept_threshold,
                &data_dir,
                &output_dir,
                seed.unwrap_or(42),
            )?;
        }
    }

    Ok(())
}
