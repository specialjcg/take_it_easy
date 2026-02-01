//! Benchmark Graph Attention Network (GAT) vs GNN
//!
//! This script:
//! 1. Generates training data from games with greedy policy
//! 2. Trains GAT and GNN models
//! 3. Compares their performance
//!
//! Usage: cargo run --release --bin benchmark_gat -- --games 50 --epochs 10

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::time::Instant;
use tch::{nn, nn::OptimizerConfig, Device, IndexOp, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::gat::GATPolicyNet;
use take_it_easy::neural::gnn::GraphPolicyNet;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gnn_with_tile;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "benchmark_gat")]
#[command(about = "Benchmark Graph Attention Network for Take It Easy")]
struct Args {
    /// Number of games to generate for training data
    #[arg(long, default_value_t = 100)]
    games: usize,

    /// Number of training epochs
    #[arg(long, default_value_t = 20)]
    epochs: usize,

    /// Number of attention heads for GAT
    #[arg(long, default_value_t = 4)]
    num_heads: usize,

    /// Hidden dimensions
    #[arg(long, default_value_t = 64)]
    hidden_dim: i64,

    /// Number of layers
    #[arg(long, default_value_t = 2)]
    num_layers: usize,

    /// Learning rate
    #[arg(long, default_value_t = 0.001)]
    lr: f64,

    /// Dropout rate
    #[arg(long, default_value_t = 0.1)]
    dropout: f64,

    /// Number of evaluation games
    #[arg(long, default_value_t = 50)]
    eval_games: usize,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

/// Training sample: (state, best_position, final_score)
struct TrainingSample {
    features: Tensor,      // [19, 8] node features
    best_pos: i64,         // Best position (0-18)
    _value: f32,           // Normalized score (for future use)
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║   Graph Attention Network (GAT) Benchmark for Take It Easy   ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Device
    let device = Device::Cpu;
    println!("📍 Device: {:?}", device);
    println!("📊 Config: {} heads, {} hidden dim, {} layers\n",
             args.num_heads, args.hidden_dim, args.num_layers);

    // Generate training data
    println!("📊 Generating training data from {} games...", args.games);
    let start = Instant::now();
    let training_data = generate_training_data(args.games, args.seed);
    println!(
        "   Generated {} samples in {:.2}s\n",
        training_data.len(),
        start.elapsed().as_secs_f32()
    );

    // Build hidden dims
    let hidden_dims: Vec<i64> = vec![args.hidden_dim; args.num_layers];

    // Train and evaluate GAT
    println!("🔷 Training GAT (Graph Attention Network)...");
    let gat_score = train_and_evaluate_gat(
        &training_data,
        &hidden_dims,
        args.num_heads,
        args.dropout,
        args.epochs,
        args.lr,
        args.eval_games,
        args.seed,
        device,
    );

    // Train and evaluate GNN
    println!("\n🔶 Training GNN (Graph Neural Network)...");
    let gnn_score = train_and_evaluate_gnn(
        &training_data,
        &hidden_dims,
        args.dropout,
        args.epochs,
        args.lr,
        args.eval_games,
        args.seed,
        device,
    );

    // Baseline: random play
    println!("\n⚪ Baseline: Random play...");
    let random_score = evaluate_random(args.eval_games, args.seed);

    // Baseline: greedy play
    println!("\n🟢 Baseline: Greedy play...");
    let greedy_score = evaluate_greedy(args.eval_games, args.seed);

    // Results
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                        RESULTS                                ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Model          │  Avg Score  │  vs Random  │  vs Greedy     ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║  GAT            │  {:>7.2}    │  {:>+7.2}    │  {:>+7.2}        ║",
        gat_score,
        gat_score - random_score,
        gat_score - greedy_score
    );
    println!(
        "║  GNN            │  {:>7.2}    │  {:>+7.2}    │  {:>+7.2}        ║",
        gnn_score,
        gnn_score - random_score,
        gnn_score - greedy_score
    );
    println!(
        "║  Greedy         │  {:>7.2}    │  {:>+7.2}    │     -          ║",
        greedy_score,
        greedy_score - random_score
    );
    println!(
        "║  Random         │  {:>7.2}    │     -       │     -          ║",
        random_score
    );
    println!("╚══════════════════════════════════════════════════════════════╝");

    // Verdict
    println!("\n📈 Verdict:");
    let delta = gat_score - gnn_score;
    if delta > 5.0 {
        println!("   ✅ GAT significantly outperforms GNN (+{:.2} pts)", delta);
    } else if delta > 0.0 {
        println!("   🔶 GAT slightly better than GNN (+{:.2} pts)", delta);
    } else if delta < -5.0 {
        println!("   ❌ GNN outperforms GAT (+{:.2} pts)", -delta);
    } else {
        println!("   🔶 GAT and GNN perform similarly ({:.2} pts difference)", delta.abs());
    }

    // Stability check
    println!("\n📊 Stability Analysis:");
    println!("   Running 3 additional evaluations to check variance...");

    let mut gat_scores = vec![gat_score];
    let mut gnn_scores = vec![gnn_score];

    for i in 1..=3 {
        let seed = args.seed + i as u64 * 1000;
        let gat_s = evaluate_with_gat_policy_seed(&training_data, &hidden_dims, args.num_heads, args.dropout, args.epochs, args.lr, args.eval_games, seed, device);
        let gnn_s = evaluate_with_gnn_policy_seed(&training_data, &hidden_dims, args.dropout, args.epochs, args.lr, args.eval_games, seed, device);
        gat_scores.push(gat_s);
        gnn_scores.push(gnn_s);
    }

    let gat_mean: f64 = gat_scores.iter().sum::<f64>() / gat_scores.len() as f64;
    let gnn_mean: f64 = gnn_scores.iter().sum::<f64>() / gnn_scores.len() as f64;
    let gat_std = (gat_scores.iter().map(|x| (x - gat_mean).powi(2)).sum::<f64>() / gat_scores.len() as f64).sqrt();
    let gnn_std = (gnn_scores.iter().map(|x| (x - gnn_mean).powi(2)).sum::<f64>() / gnn_scores.len() as f64).sqrt();

    println!("   GAT: {:.2} ± {:.2}", gat_mean, gat_std);
    println!("   GNN: {:.2} ± {:.2}", gnn_mean, gnn_std);

    if gat_std < gnn_std {
        println!("   ✅ GAT is more stable (lower variance)");
    } else if gnn_std < gat_std {
        println!("   ⚠️  GNN is more stable (lower variance)");
    } else {
        println!("   Similar stability");
    }
}

/// Generate training data by playing games with greedy policy
fn generate_training_data(num_games: usize, seed: u64) -> Vec<TrainingSample> {
    let mut samples = Vec::new();
    let mut rng = StdRng::seed_from_u64(seed);

    for game_idx in 0..num_games {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();

        let mut game_samples = Vec::new();

        for turn in 0..19 {
            let available_tiles = get_available_tiles(&deck);
            if available_tiles.is_empty() {
                break;
            }

            // Pick a random tile
            let tile = *available_tiles.choose(&mut rng).unwrap();

            // Find available positions
            let available_positions: Vec<usize> = (0..19)
                .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
                .collect();

            if available_positions.is_empty() {
                break;
            }

            // Convert state to features
            let features = convert_plateau_for_gnn_with_tile(&plateau, &tile, turn, 19);

            // Find best position using greedy scoring
            let best_pos = find_best_position_greedy(&plateau, &tile, &available_positions);

            game_samples.push((features, best_pos as i64));

            // Place tile at best position
            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        // Calculate final score and normalize
        let final_score = result(&plateau);
        let normalized_score = (final_score as f32 - 50.0) / 150.0;

        // Add samples with value labels
        for (features, best_pos) in game_samples {
            samples.push(TrainingSample {
                features,
                best_pos,
                _value: normalized_score,
            });
        }

        if (game_idx + 1) % 20 == 0 {
            print!("   Game {}/{}\r", game_idx + 1, num_games);
        }
    }

    println!();
    samples
}

/// Find best position using greedy line completion heuristic
fn find_best_position_greedy(plateau: &take_it_easy::game::plateau::Plateau, tile: &Tile, available: &[usize]) -> usize {
    let mut best_pos = available[0];
    let mut best_score = i32::MIN;

    for &pos in available {
        let mut test_plateau = plateau.clone();
        test_plateau.tiles[pos] = *tile;
        let score = result(&test_plateau);

        if score > best_score {
            best_score = score;
            best_pos = pos;
        }
    }

    best_pos
}

/// Train and evaluate GAT
fn train_and_evaluate_gat(
    data: &[TrainingSample],
    hidden_dims: &[i64],
    num_heads: usize,
    dropout: f64,
    epochs: usize,
    lr: f64,
    eval_games: usize,
    seed: u64,
    device: Device,
) -> f64 {
    let vs = nn::VarStore::new(device);
    let policy_net = GATPolicyNet::new(&vs, 8, hidden_dims, num_heads, dropout);

    let mut opt = nn::Adam::default().build(&vs, lr).unwrap();

    // Training loop
    let batch_size = 32;
    let num_batches = data.len() / batch_size;
    let mut rng = StdRng::seed_from_u64(seed);

    for epoch in 0..epochs {
        let mut epoch_loss = 0.0;
        let mut indices: Vec<usize> = (0..data.len()).collect();
        indices.shuffle(&mut rng);

        for batch_idx in 0..num_batches {
            let batch_indices = &indices[batch_idx * batch_size..(batch_idx + 1) * batch_size];

            // Collect batch
            let features: Vec<Tensor> = batch_indices
                .iter()
                .map(|&i| data[i].features.shallow_clone())
                .collect();
            let targets: Vec<i64> = batch_indices.iter().map(|&i| data[i].best_pos).collect();

            let batch_features = Tensor::stack(&features, 0);
            let batch_targets = Tensor::from_slice(&targets);

            // Forward pass
            let logits = policy_net.forward(&batch_features, true);

            // Cross-entropy loss
            let loss = logits.cross_entropy_for_logits(&batch_targets);

            // Backward pass
            opt.backward_step(&loss);

            epoch_loss += f64::try_from(&loss).unwrap();
        }

        let avg_loss = epoch_loss / num_batches as f64;
        if (epoch + 1) % 5 == 0 || epoch == 0 {
            println!("   Epoch {}/{}: loss = {:.4}", epoch + 1, epochs, avg_loss);
        }
    }

    // Evaluate
    evaluate_with_policy(&policy_net, eval_games, seed, "GAT")
}

/// Train and evaluate GNN
fn train_and_evaluate_gnn(
    data: &[TrainingSample],
    hidden_dims: &[i64],
    dropout: f64,
    epochs: usize,
    lr: f64,
    eval_games: usize,
    seed: u64,
    device: Device,
) -> f64 {
    let vs = nn::VarStore::new(device);
    let policy_net = GraphPolicyNet::new(&vs, 8, hidden_dims, dropout);

    let mut opt = nn::Adam::default().build(&vs, lr).unwrap();

    // Training loop
    let batch_size = 32;
    let num_batches = data.len() / batch_size;
    let mut rng = StdRng::seed_from_u64(seed);

    for epoch in 0..epochs {
        let mut epoch_loss = 0.0;
        let mut indices: Vec<usize> = (0..data.len()).collect();
        indices.shuffle(&mut rng);

        for batch_idx in 0..num_batches {
            let batch_indices = &indices[batch_idx * batch_size..(batch_idx + 1) * batch_size];

            // Collect batch
            let features: Vec<Tensor> = batch_indices
                .iter()
                .map(|&i| data[i].features.shallow_clone())
                .collect();
            let targets: Vec<i64> = batch_indices.iter().map(|&i| data[i].best_pos).collect();

            let batch_features = Tensor::stack(&features, 0);
            let batch_targets = Tensor::from_slice(&targets);

            // Forward pass
            let logits = policy_net.forward(&batch_features, true);

            // Cross-entropy loss
            let loss = logits.cross_entropy_for_logits(&batch_targets);

            // Backward pass
            opt.backward_step(&loss);

            epoch_loss += f64::try_from(&loss).unwrap();
        }

        let avg_loss = epoch_loss / num_batches as f64;
        if (epoch + 1) % 5 == 0 || epoch == 0 {
            println!("   Epoch {}/{}: loss = {:.4}", epoch + 1, epochs, avg_loss);
        }
    }

    // Evaluate
    evaluate_with_gnn_policy(&policy_net, eval_games, seed)
}

/// Evaluate GAT policy in games
fn evaluate_with_policy(policy: &GATPolicyNet, num_games: usize, seed: u64, name: &str) -> f64 {
    let mut total_score = 0;
    let mut rng = StdRng::seed_from_u64(seed + 10000);

    for _ in 0..num_games {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();

        for turn in 0..19 {
            let available_tiles = get_available_tiles(&deck);
            if available_tiles.is_empty() {
                break;
            }

            let tile = *available_tiles.choose(&mut rng).unwrap();

            let available_positions: Vec<usize> = (0..19)
                .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
                .collect();

            if available_positions.is_empty() {
                break;
            }

            // Get policy prediction
            let features = convert_plateau_for_gnn_with_tile(&plateau, &tile, turn, 19);
            let features_batch = features.unsqueeze(0);
            let logits = policy.forward(&features_batch, false);

            // Mask unavailable positions and select best
            let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
            for &pos in &available_positions {
                let _ = mask.i(pos as i64).fill_(0.0);
            }
            let masked_logits = logits.squeeze_dim(0) + mask;
            let best_pos: i64 = masked_logits.argmax(0, false).try_into().unwrap();

            plateau.tiles[best_pos as usize] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        total_score += result(&plateau);
    }

    let avg = total_score as f64 / num_games as f64;
    println!("   {} evaluation: {:.2} pts average over {} games", name, avg, num_games);
    avg
}

/// Evaluate GNN policy in games
fn evaluate_with_gnn_policy(policy: &GraphPolicyNet, num_games: usize, seed: u64) -> f64 {
    let mut total_score = 0;
    let mut rng = StdRng::seed_from_u64(seed + 10000);

    for _ in 0..num_games {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();

        for turn in 0..19 {
            let available_tiles = get_available_tiles(&deck);
            if available_tiles.is_empty() {
                break;
            }

            let tile = *available_tiles.choose(&mut rng).unwrap();

            let available_positions: Vec<usize> = (0..19)
                .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
                .collect();

            if available_positions.is_empty() {
                break;
            }

            // Get policy prediction
            let features = convert_plateau_for_gnn_with_tile(&plateau, &tile, turn, 19);
            let features_batch = features.unsqueeze(0);
            let logits = policy.forward(&features_batch, false);

            // Mask unavailable positions and select best
            let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
            for &pos in &available_positions {
                let _ = mask.i(pos as i64).fill_(0.0);
            }
            let masked_logits = logits.squeeze_dim(0) + mask;
            let best_pos: i64 = masked_logits.argmax(0, false).try_into().unwrap();

            plateau.tiles[best_pos as usize] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        total_score += result(&plateau);
    }

    let avg = total_score as f64 / num_games as f64;
    println!("   GNN evaluation: {:.2} pts average over {} games", avg, num_games);
    avg
}

/// Quick evaluation for stability check
fn evaluate_with_gat_policy_seed(
    data: &[TrainingSample],
    hidden_dims: &[i64],
    num_heads: usize,
    dropout: f64,
    epochs: usize,
    lr: f64,
    eval_games: usize,
    seed: u64,
    device: Device,
) -> f64 {
    let vs = nn::VarStore::new(device);
    let policy_net = GATPolicyNet::new(&vs, 8, hidden_dims, num_heads, dropout);
    let mut opt = nn::Adam::default().build(&vs, lr).unwrap();

    let batch_size = 32;
    let num_batches = data.len() / batch_size;
    let mut rng = StdRng::seed_from_u64(seed);

    for _epoch in 0..epochs {
        let mut indices: Vec<usize> = (0..data.len()).collect();
        indices.shuffle(&mut rng);

        for batch_idx in 0..num_batches {
            let batch_indices = &indices[batch_idx * batch_size..(batch_idx + 1) * batch_size];
            let features: Vec<Tensor> = batch_indices.iter().map(|&i| data[i].features.shallow_clone()).collect();
            let targets: Vec<i64> = batch_indices.iter().map(|&i| data[i].best_pos).collect();
            let batch_features = Tensor::stack(&features, 0);
            let batch_targets = Tensor::from_slice(&targets);
            let logits = policy_net.forward(&batch_features, true);
            let loss = logits.cross_entropy_for_logits(&batch_targets);
            opt.backward_step(&loss);
        }
    }

    // Evaluate silently
    let mut total_score = 0;
    let mut rng = StdRng::seed_from_u64(seed + 10000);
    for _ in 0..eval_games {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();
        for turn in 0..19 {
            let available_tiles = get_available_tiles(&deck);
            if available_tiles.is_empty() { break; }
            let tile = *available_tiles.choose(&mut rng).unwrap();
            let available_positions: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if available_positions.is_empty() { break; }
            let features = convert_plateau_for_gnn_with_tile(&plateau, &tile, turn, 19);
            let logits = policy_net.forward(&features.unsqueeze(0), false);
            let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
            for &pos in &available_positions { let _ = mask.i(pos as i64).fill_(0.0); }
            let best_pos: i64 = (logits.squeeze_dim(0) + mask).argmax(0, false).try_into().unwrap();
            plateau.tiles[best_pos as usize] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        total_score += result(&plateau);
    }
    total_score as f64 / eval_games as f64
}

fn evaluate_with_gnn_policy_seed(
    data: &[TrainingSample],
    hidden_dims: &[i64],
    dropout: f64,
    epochs: usize,
    lr: f64,
    eval_games: usize,
    seed: u64,
    device: Device,
) -> f64 {
    let vs = nn::VarStore::new(device);
    let policy_net = GraphPolicyNet::new(&vs, 8, hidden_dims, dropout);
    let mut opt = nn::Adam::default().build(&vs, lr).unwrap();

    let batch_size = 32;
    let num_batches = data.len() / batch_size;
    let mut rng = StdRng::seed_from_u64(seed);

    for _epoch in 0..epochs {
        let mut indices: Vec<usize> = (0..data.len()).collect();
        indices.shuffle(&mut rng);
        for batch_idx in 0..num_batches {
            let batch_indices = &indices[batch_idx * batch_size..(batch_idx + 1) * batch_size];
            let features: Vec<Tensor> = batch_indices.iter().map(|&i| data[i].features.shallow_clone()).collect();
            let targets: Vec<i64> = batch_indices.iter().map(|&i| data[i].best_pos).collect();
            let batch_features = Tensor::stack(&features, 0);
            let batch_targets = Tensor::from_slice(&targets);
            let logits = policy_net.forward(&batch_features, true);
            let loss = logits.cross_entropy_for_logits(&batch_targets);
            opt.backward_step(&loss);
        }
    }

    let mut total_score = 0;
    let mut rng = StdRng::seed_from_u64(seed + 10000);
    for _ in 0..eval_games {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();
        for turn in 0..19 {
            let available_tiles = get_available_tiles(&deck);
            if available_tiles.is_empty() { break; }
            let tile = *available_tiles.choose(&mut rng).unwrap();
            let available_positions: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if available_positions.is_empty() { break; }
            let features = convert_plateau_for_gnn_with_tile(&plateau, &tile, turn, 19);
            let logits = policy_net.forward(&features.unsqueeze(0), false);
            let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
            for &pos in &available_positions { let _ = mask.i(pos as i64).fill_(0.0); }
            let best_pos: i64 = (logits.squeeze_dim(0) + mask).argmax(0, false).try_into().unwrap();
            plateau.tiles[best_pos as usize] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        total_score += result(&plateau);
    }
    total_score as f64 / eval_games as f64
}

/// Evaluate random play baseline
fn evaluate_random(num_games: usize, seed: u64) -> f64 {
    let mut total_score = 0;
    let mut rng = StdRng::seed_from_u64(seed + 20000);

    for _ in 0..num_games {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();

        for _turn in 0..19 {
            let available_tiles = get_available_tiles(&deck);
            if available_tiles.is_empty() {
                break;
            }

            let tile = *available_tiles.choose(&mut rng).unwrap();

            let available_positions: Vec<usize> = (0..19)
                .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
                .collect();

            if available_positions.is_empty() {
                break;
            }

            let pos = *available_positions.choose(&mut rng).unwrap();
            plateau.tiles[pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        total_score += result(&plateau);
    }

    let avg = total_score as f64 / num_games as f64;
    println!("   Random baseline: {:.2} pts average over {} games", avg, num_games);
    avg
}

/// Evaluate greedy play baseline
fn evaluate_greedy(num_games: usize, seed: u64) -> f64 {
    let mut total_score = 0;
    let mut rng = StdRng::seed_from_u64(seed + 30000);

    for _ in 0..num_games {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();

        for _turn in 0..19 {
            let available_tiles = get_available_tiles(&deck);
            if available_tiles.is_empty() {
                break;
            }

            let tile = *available_tiles.choose(&mut rng).unwrap();

            let available_positions: Vec<usize> = (0..19)
                .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
                .collect();

            if available_positions.is_empty() {
                break;
            }

            let best_pos = find_best_position_greedy(&plateau, &tile, &available_positions);
            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        total_score += result(&plateau);
    }

    let avg = total_score as f64 / num_games as f64;
    println!("   Greedy baseline: {:.2} pts average over {} games", avg, num_games);
    avg
}
