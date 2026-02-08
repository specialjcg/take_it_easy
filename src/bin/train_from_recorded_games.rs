//! Train Graph Transformer from Recorded Human Games
//!
//! This script trains the policy network using games recorded from human play.
//! Games where the human beat the AI are weighted more heavily (3x by default).
//!
//! Usage: cargo run --release --bin train_from_recorded_games -- --epochs 50

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
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::graph_transformer::GraphTransformerPolicyNet;
use take_it_easy::neural::model_io::{load_varstore, save_varstore};
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;

#[derive(Parser, Debug)]
#[command(name = "train_from_recorded_games")]
struct Args {
    /// Directory containing recorded games
    #[arg(long, default_value = "data/recorded_games")]
    data_dir: String,

    /// Weight multiplier for games where human won
    #[arg(long, default_value_t = 3.0)]
    human_win_weight: f64,

    /// Base weight for games where AI won
    #[arg(long, default_value_t = 1.0)]
    ai_win_weight: f64,

    /// Training epochs
    #[arg(long, default_value_t = 50)]
    epochs: usize,

    /// Batch size
    #[arg(long, default_value_t = 32)]
    batch_size: usize,

    /// Learning rate
    #[arg(long, default_value_t = 0.0001)]
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

    /// Load existing model weights
    #[arg(long, default_value = "model_weights/graph_transformer_policy.safetensors")]
    load_path: String,

    /// Save model path
    #[arg(long, default_value = "model_weights/graph_transformer_human")]
    save_path: String,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Only use human moves (not AI moves)
    #[arg(long, default_value_t = true)]
    human_moves_only: bool,

    /// Minimum score to include
    #[arg(long, default_value_t = 80)]
    min_score: i32,
}

#[derive(Clone)]
struct Sample {
    plateau: [i32; 19],
    tile: (i32, i32, i32),
    position: usize,
    turn: usize,
    final_score: i32,
    human_won: bool,
    weight: f64,
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     Train from Recorded Human Games                          ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("Config:");
    println!("  Data dir:         {}", args.data_dir);
    println!("  Human win weight: {:.1}x", args.human_win_weight);
    println!("  AI win weight:    {:.1}x", args.ai_win_weight);
    println!("  Human moves only: {}", args.human_moves_only);
    println!("  Min score:        {}", args.min_score);
    println!("  Epochs:           {}", args.epochs);
    println!("  Learning rate:    {}", args.lr);
    println!("  Load from:        {}", args.load_path);

    // Load recorded games
    println!("\n📂 Loading recorded games from {}...", args.data_dir);
    let samples = load_recorded_games(
        &args.data_dir,
        args.human_moves_only,
        args.min_score,
        args.human_win_weight,
        args.ai_win_weight,
    );

    if samples.is_empty() {
        println!("❌ No samples found! Play some games first.");
        return;
    }

    // Statistics
    let human_won_count = samples.iter().filter(|s| s.human_won).count();
    let ai_won_count = samples.len() - human_won_count;
    let total_weight: f64 = samples.iter().map(|s| s.weight).sum();

    println!("   Loaded {} samples", samples.len());
    println!("   Human won games: {} ({:.1}%)", human_won_count, human_won_count as f64 / samples.len() as f64 * 100.0);
    println!("   AI won games:    {} ({:.1}%)", ai_won_count, ai_won_count as f64 / samples.len() as f64 * 100.0);
    println!("   Total weight:    {:.1}", total_weight);

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

    // Load existing weights if available
    if Path::new(&args.load_path).exists() {
        match load_varstore(&mut vs, &args.load_path) {
            Ok(()) => println!("✅ Loaded existing weights from {}", args.load_path),
            Err(e) => println!("⚠️ Could not load weights: {} - starting fresh", e),
        }
    } else {
        println!("ℹ️ No existing weights found - training from scratch");
    }

    let mut opt = nn::Adam::default().build(&vs, args.lr).unwrap();
    let mut rng = StdRng::seed_from_u64(args.seed);

    // Shuffle and split
    let mut indices: Vec<usize> = (0..samples.len()).collect();
    indices.shuffle(&mut rng);

    let val_size = (samples.len() as f64 * 0.1) as usize;
    let (val_idx, train_idx) = indices.split_at(val_size);
    let train_idx: Vec<usize> = train_idx.to_vec();
    let val_indices: Vec<usize> = val_idx.to_vec();

    println!("\n🏋️ Training...\n");
    println!("  Train samples: {}", train_idx.len());
    println!("  Val samples:   {}", val_indices.len());

    let mut best_val_acc = 0.0;
    let mut best_game_score = 0.0;

    for epoch in 0..args.epochs {
        let epoch_start = Instant::now();
        let mut shuffled = train_idx.clone();
        shuffled.shuffle(&mut rng);

        let n_batches = shuffled.len() / args.batch_size;
        if n_batches == 0 {
            println!("Not enough samples for a batch!");
            return;
        }

        let mut train_loss = 0.0;
        let mut train_correct = 0usize;

        for batch_i in 0..n_batches {
            let batch_indices: Vec<usize> =
                shuffled[batch_i * args.batch_size..(batch_i + 1) * args.batch_size].to_vec();

            let (features, targets, masks, weights) = prepare_batch_weighted(&samples, &batch_indices);

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
        let (val_loss, val_acc) = evaluate(&policy_net, &samples, &val_indices, args.batch_size);

        let elapsed = epoch_start.elapsed().as_secs_f32();

        // Game evaluation every 10 epochs
        let should_eval = epoch % 10 == 9 || epoch == args.epochs - 1;
        let game_score = if should_eval {
            let (score, _) = eval_games(&policy_net, 100, &mut rng);
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
                "Epoch {:3}/{:3} | Train Loss: {:.4}, Acc: {:.2}% | Val Acc: {:.2}% | {:.1}s{}{}",
                epoch + 1,
                args.epochs,
                train_loss,
                train_acc * 100.0,
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
            let path = format!("{}_policy.safetensors", args.save_path);
            if let Err(e) = save_varstore(&vs, &path) {
                eprintln!("Warning: failed to save: {}", e);
            }
            println!("   📁 New best game score! Model saved to {}", path);
        }
    }

    println!("\n══════════════════════════════════════════════════════════════");
    println!("                     TRAINING COMPLETE");
    println!("══════════════════════════════════════════════════════════════");
    println!("\n  Best validation accuracy: {:.2}%", best_val_acc * 100.0);
    println!("  Best game score: {:.2} pts", best_game_score);

    // Final evaluation
    println!("\n🎮 Evaluating by playing 200 games...\n");
    let (gt_avg, gt_scores) = eval_games(&policy_net, 200, &mut rng);

    println!("  Graph Transformer (Human-tuned): {:.2} pts", gt_avg);

    let above_100: usize = gt_scores.iter().filter(|&&s| s >= 100).count();
    let above_140: usize = gt_scores.iter().filter(|&&s| s >= 140).count();
    let above_150: usize = gt_scores.iter().filter(|&&s| s >= 150).count();
    println!("\n  Games >= 100 pts: {} ({:.1}%)", above_100, above_100 as f64 / 200.0 * 100.0);
    println!("  Games >= 140 pts: {} ({:.1}%)", above_140, above_140 as f64 / 200.0 * 100.0);
    println!("  Games >= 150 pts: {} ({:.1}%)", above_150, above_150 as f64 / 200.0 * 100.0);
}

fn load_recorded_games(
    dir: &str,
    human_moves_only: bool,
    min_score: i32,
    human_win_weight: f64,
    ai_win_weight: f64,
) -> Vec<Sample> {
    let mut samples = Vec::new();
    let path = Path::new(dir);
    if !path.exists() {
        return samples;
    }

    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let file_path = entry.path();
        if file_path.extension().map_or(false, |e| e == "csv") {
            samples.extend(load_csv_recorded(
                &file_path,
                human_moves_only,
                min_score,
                human_win_weight,
                ai_win_weight,
            ));
        }
    }

    samples
}

fn load_csv_recorded(
    path: &Path,
    human_moves_only: bool,
    min_score: i32,
    human_win_weight: f64,
    ai_win_weight: f64,
) -> Vec<Sample> {
    let mut samples = Vec::new();
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return samples,
    };
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    // Skip header
    let _ = lines.next();

    for line in lines {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 28 {
            continue;
        }

        // Parse player_type
        let player_type = parts[2];
        if human_moves_only && player_type != "Human" {
            continue;
        }

        // CSV columns: game_id(0), turn(1), player_type(2), plateau_0-18 (3-21),
        // tile_0-2 (22-24), position(25), final_score(26), human_won(27)
        // Total: 28 columns

        // Parse plateau
        let mut plateau = [0i32; 19];
        for i in 0..19 {
            plateau[i] = parts[3 + i].parse().unwrap_or(0);
        }

        // Parse tile (indices 22, 23, 24)
        let tile = (
            parts[22].parse().unwrap_or(0),
            parts[23].parse().unwrap_or(0),
            parts[24].parse().unwrap_or(0),
        );

        // Parse position (index 25)
        let position: usize = parts[25].parse().unwrap_or(0);

        // Parse final_score (index 26) and human_won (index 27)
        let final_score: i32 = parts[26].parse().unwrap_or(0);
        let human_won: bool = if parts.len() > 27 {
            parts[27].parse::<i32>().unwrap_or(0) == 1
        } else {
            false
        };

        if final_score < min_score {
            continue;
        }

        // Parse turn
        let turn: usize = parts[1].parse().unwrap_or(0);

        // Weight based on whether human won
        let weight = if human_won {
            human_win_weight * (final_score as f64 / 100.0)
        } else {
            ai_win_weight * (final_score as f64 / 100.0)
        };

        samples.push(Sample {
            plateau,
            tile,
            position,
            turn,
            final_score,
            human_won,
            weight,
        });
    }

    samples
}

fn prepare_batch_weighted(samples: &[Sample], indices: &[usize]) -> (Tensor, Tensor, Tensor, Tensor) {
    let batch_size = indices.len();
    let mut features_vec = Vec::with_capacity(batch_size);
    let mut targets = Vec::with_capacity(batch_size);
    let mut masks_vec = Vec::with_capacity(batch_size);
    let mut weights_vec = Vec::with_capacity(batch_size);

    let deck = create_deck();

    for &idx in indices {
        let sample = &samples[idx];

        // Convert plateau to Plateau struct
        let mut plateau = Plateau {
            tiles: vec![Tile(0, 0, 0); 19],
        };
        for i in 0..19 {
            let v = sample.plateau[i];
            if v > 0 {
                // Decode: v = v1 * 100 + v2 * 10 + v3
                let v1 = (v / 100) as i32;
                let v2 = ((v / 10) % 10) as i32;
                let v3 = (v % 10) as i32;
                plateau.tiles[i] = Tile(v1, v2, v3);
            }
        }

        let tile = Tile(sample.tile.0, sample.tile.1, sample.tile.2);

        // Convert to tensor
        let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, sample.turn, 19);
        features_vec.push(feat);

        targets.push(sample.position as i64);
        weights_vec.push(sample.weight as f32);

        // Create mask (0 for available, -inf for occupied)
        let mut mask = [0.0f32; 19];
        for i in 0..19 {
            if sample.plateau[i] != 0 {
                mask[i] = f32::NEG_INFINITY;
            }
        }
        masks_vec.push(Tensor::from_slice(&mask));
    }

    let features = Tensor::stack(&features_vec, 0);  // [batch, 19, 47]
    let targets = Tensor::from_slice(&targets);
    let masks = Tensor::stack(&masks_vec, 0);  // [batch, 19]
    let weights = Tensor::from_slice(&weights_vec);

    (features, targets, masks, weights)
}

fn evaluate(
    policy_net: &GraphTransformerPolicyNet,
    samples: &[Sample],
    indices: &[usize],
    batch_size: usize,
) -> (f64, f64) {
    let n_batches = indices.len() / batch_size;
    if n_batches == 0 {
        return (0.0, 0.0);
    }

    let mut total_loss = 0.0;
    let mut total_correct = 0usize;

    for batch_i in 0..n_batches {
        let batch_indices: Vec<usize> =
            indices[batch_i * batch_size..(batch_i + 1) * batch_size].to_vec();

        let (features, targets, masks, weights) = prepare_batch_weighted(samples, &batch_indices);

        let logits = policy_net.forward(&features, false);
        let masked_logits = logits + &masks;
        let log_probs = masked_logits.log_softmax(-1, Kind::Float);

        let per_sample_loss = -log_probs.gather(1, &targets.unsqueeze(1), false).squeeze_dim(1);
        let loss = (&per_sample_loss * &weights).sum(Kind::Float) / weights.sum(Kind::Float);
        total_loss += f64::try_from(&loss).unwrap();

        let preds = masked_logits.argmax(-1, false);
        let correct: i64 = preds.eq_tensor(&targets).sum(Kind::Int64).int64_value(&[]);
        total_correct += correct as usize;
    }

    let avg_loss = total_loss / n_batches as f64;
    let accuracy = total_correct as f64 / (n_batches * batch_size) as f64;

    (avg_loss, accuracy)
}

fn eval_games(policy_net: &GraphTransformerPolicyNet, n_games: usize, rng: &mut StdRng) -> (f64, Vec<i32>) {
    use take_it_easy::game::get_legal_moves::get_legal_moves;
    use take_it_easy::game::plateau::create_plateau_empty;
    use take_it_easy::scoring::scoring::result;

    let mut scores = Vec::with_capacity(n_games);

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut available: Vec<Tile> = deck.tiles().iter().copied().filter(|t| *t != Tile(0, 0, 0)).collect();

        for turn in 0..19 {
            if available.is_empty() {
                break;
            }

            let tile_idx = rng.random_range(0..available.len());
            let tile = available.remove(tile_idx);
            deck = take_it_easy::game::remove_tile_from_deck::replace_tile_in_deck(&deck, &tile);

            let legal = get_legal_moves(&plateau);
            if legal.is_empty() {
                break;
            }

            // Get policy prediction - add batch dimension
            let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19).unsqueeze(0);
            let logits = policy_net.forward(&feat, false).squeeze_dim(0);  // Remove batch dim -> [19]

            // Mask illegal moves
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
