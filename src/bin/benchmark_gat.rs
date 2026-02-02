//! Benchmark Graph Attention Network (GAT) with 47 channels vs GNN with 8 channels
//!
//! GAT uses enriched features including explicit line information,
//! while GNN uses basic node features.
//!
//! Usage: cargo run --release --bin benchmark_gat -- --games 200 --epochs 50

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
use take_it_easy::neural::tensor_conversion::{convert_plateau_for_gat_47ch, convert_plateau_for_gnn_with_tile};
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "benchmark_gat")]
#[command(about = "Benchmark GAT (47ch) vs GNN (8ch) for Take It Easy")]
struct Args {
    /// Number of games to generate for training data
    #[arg(long, default_value_t = 200)]
    games: usize,

    /// Number of training epochs
    #[arg(long, default_value_t = 50)]
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
    #[arg(long, default_value_t = 100)]
    eval_games: usize,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

/// Training sample for GAT (47 channels)
struct GATSample {
    features: Tensor,      // [19, 47] node features
    best_pos: i64,
}

/// Training sample for GNN (8 channels)
struct GNNSample {
    features: Tensor,      // [19, 8] node features
    best_pos: i64,
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     GAT (47 channels) vs GNN (8 channels) Benchmark          ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let device = Device::Cpu;
    println!("📍 Device: {:?}", device);
    println!("📊 GAT: 47 input features (with line info)");
    println!("📊 GNN: 8 input features (basic)");
    println!("📊 Config: {} heads, {} hidden dim, {} layers\n",
             args.num_heads, args.hidden_dim, args.num_layers);

    // Generate training data
    println!("📊 Generating training data from {} games...", args.games);
    let start = Instant::now();
    let (gat_data, gnn_data) = generate_training_data(args.games, args.seed);
    println!(
        "   Generated {} samples in {:.2}s\n",
        gat_data.len(),
        start.elapsed().as_secs_f32()
    );

    let hidden_dims: Vec<i64> = vec![args.hidden_dim; args.num_layers];

    // Train and evaluate GAT with 47 channels
    println!("🔷 Training GAT (47 channels with line features)...");
    let gat_score = train_and_evaluate_gat(
        &gat_data,
        &hidden_dims,
        args.num_heads,
        args.dropout,
        args.epochs,
        args.lr,
        args.eval_games,
        args.seed,
        device,
    );

    // Train and evaluate GNN with 8 channels
    println!("\n🔶 Training GNN (8 channels basic)...");
    let gnn_score = train_and_evaluate_gnn(
        &gnn_data,
        &hidden_dims,
        args.dropout,
        args.epochs,
        args.lr,
        args.eval_games,
        args.seed,
        device,
    );

    // Baselines
    println!("\n⚪ Baseline: Random play...");
    let random_score = evaluate_random(args.eval_games, args.seed);

    println!("\n🟢 Baseline: Greedy play...");
    let greedy_score = evaluate_greedy(args.eval_games, args.seed);

    // Results
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                        RESULTS                                ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Model              │  Avg Score  │  vs Random  │ vs Greedy  ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║  GAT (47ch)         │  {:>7.2}    │  {:>+7.2}    │  {:>+7.2}    ║",
        gat_score,
        gat_score - random_score,
        gat_score - greedy_score
    );
    println!(
        "║  GNN (8ch)          │  {:>7.2}    │  {:>+7.2}    │  {:>+7.2}    ║",
        gnn_score,
        gnn_score - random_score,
        gnn_score - greedy_score
    );
    println!(
        "║  Greedy             │  {:>7.2}    │  {:>+7.2}    │     -      ║",
        greedy_score,
        greedy_score - random_score
    );
    println!(
        "║  Random             │  {:>7.2}    │     -       │     -      ║",
        random_score
    );
    println!("╚══════════════════════════════════════════════════════════════╝");

    // Verdict
    println!("\n📈 Verdict:");
    let delta = gat_score - gnn_score;
    if delta > 10.0 {
        println!("   ✅ GAT (47ch) significantly outperforms GNN (+{:.2} pts)", delta);
        println!("   → Line features help the attention mechanism!");
    } else if delta > 3.0 {
        println!("   🔶 GAT (47ch) moderately better than GNN (+{:.2} pts)", delta);
    } else if delta > 0.0 {
        println!("   🔶 GAT (47ch) slightly better than GNN (+{:.2} pts)", delta);
    } else if delta < -10.0 {
        println!("   ❌ GNN outperforms GAT (+{:.2} pts)", -delta);
    } else {
        println!("   🔶 Similar performance ({:.2} pts difference)", delta.abs());
    }

    // Check if better than greedy
    if gat_score > greedy_score {
        println!("   ✅ GAT beats Greedy baseline!");
    }
    if gnn_score > greedy_score {
        println!("   ✅ GNN beats Greedy baseline!");
    }

    // Stability analysis
    println!("\n📊 Stability Analysis (3 runs)...");
    let mut gat_scores = vec![gat_score];
    let mut gnn_scores = vec![gnn_score];

    for i in 1..=2 {
        let seed = args.seed + i as u64 * 1000;
        let (gat_data_i, gnn_data_i) = generate_training_data(args.games, seed);

        let gat_s = train_and_eval_silent_gat(&gat_data_i, &hidden_dims, args.num_heads, args.dropout, args.epochs, args.lr, args.eval_games, seed, device);
        let gnn_s = train_and_eval_silent_gnn(&gnn_data_i, &hidden_dims, args.dropout, args.epochs, args.lr, args.eval_games, seed, device);

        gat_scores.push(gat_s);
        gnn_scores.push(gnn_s);
        print!("   Run {}/3 done\r", i + 1);
    }
    println!();

    let gat_mean: f64 = gat_scores.iter().sum::<f64>() / gat_scores.len() as f64;
    let gnn_mean: f64 = gnn_scores.iter().sum::<f64>() / gnn_scores.len() as f64;
    let gat_std = (gat_scores.iter().map(|x| (x - gat_mean).powi(2)).sum::<f64>() / gat_scores.len() as f64).sqrt();
    let gnn_std = (gnn_scores.iter().map(|x| (x - gnn_mean).powi(2)).sum::<f64>() / gnn_scores.len() as f64).sqrt();

    println!("   GAT (47ch): {:.2} ± {:.2} (scores: {:?})", gat_mean, gat_std,
             gat_scores.iter().map(|x| format!("{:.1}", x)).collect::<Vec<_>>());
    println!("   GNN (8ch):  {:.2} ± {:.2} (scores: {:?})", gnn_mean, gnn_std,
             gnn_scores.iter().map(|x| format!("{:.1}", x)).collect::<Vec<_>>());

    if gat_std < gnn_std * 0.8 {
        println!("   ✅ GAT is more stable");
    } else if gnn_std < gat_std * 0.8 {
        println!("   ⚠️  GNN is more stable");
    } else {
        println!("   Similar stability");
    }
}

/// Generate training data for both GAT (47ch) and GNN (8ch)
fn generate_training_data(num_games: usize, seed: u64) -> (Vec<GATSample>, Vec<GNNSample>) {
    let mut gat_samples = Vec::new();
    let mut gnn_samples = Vec::new();
    let mut rng = StdRng::seed_from_u64(seed);

    for game_idx in 0..num_games {
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

            // Generate features for both formats
            let gat_features = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            let gnn_features = convert_plateau_for_gnn_with_tile(&plateau, &tile, turn, 19);

            // Find best position using greedy scoring
            let best_pos = find_best_position_greedy(&plateau, &tile, &available_positions);

            gat_samples.push(GATSample {
                features: gat_features,
                best_pos: best_pos as i64,
            });

            gnn_samples.push(GNNSample {
                features: gnn_features,
                best_pos: best_pos as i64,
            });

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        if (game_idx + 1) % 50 == 0 {
            print!("   Game {}/{}\r", game_idx + 1, num_games);
        }
    }
    println!();

    (gat_samples, gnn_samples)
}

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

/// Train and evaluate GAT with 47 channels
fn train_and_evaluate_gat(
    data: &[GATSample],
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
    // GAT with 47 input features
    let policy_net = GATPolicyNet::new(&vs, 47, hidden_dims, num_heads, dropout);

    let mut opt = nn::Adam::default().build(&vs, lr).unwrap();

    let batch_size = 32;
    let num_batches = data.len() / batch_size;
    let mut rng = StdRng::seed_from_u64(seed);

    for epoch in 0..epochs {
        let mut epoch_loss = 0.0;
        let mut indices: Vec<usize> = (0..data.len()).collect();
        indices.shuffle(&mut rng);

        for batch_idx in 0..num_batches {
            let batch_indices = &indices[batch_idx * batch_size..(batch_idx + 1) * batch_size];

            let features: Vec<Tensor> = batch_indices
                .iter()
                .map(|&i| data[i].features.shallow_clone())
                .collect();
            let targets: Vec<i64> = batch_indices.iter().map(|&i| data[i].best_pos).collect();

            let batch_features = Tensor::stack(&features, 0);
            let batch_targets = Tensor::from_slice(&targets);

            let logits = policy_net.forward(&batch_features, true);
            let loss = logits.cross_entropy_for_logits(&batch_targets);

            opt.backward_step(&loss);
            epoch_loss += f64::try_from(&loss).unwrap();
        }

        let avg_loss = epoch_loss / num_batches as f64;
        if (epoch + 1) % 10 == 0 || epoch == 0 {
            println!("   Epoch {}/{}: loss = {:.4}", epoch + 1, epochs, avg_loss);
        }
    }

    evaluate_gat_policy(&policy_net, eval_games, seed, "GAT (47ch)")
}

/// Train and evaluate GNN with 8 channels
fn train_and_evaluate_gnn(
    data: &[GNNSample],
    hidden_dims: &[i64],
    dropout: f64,
    epochs: usize,
    lr: f64,
    eval_games: usize,
    seed: u64,
    device: Device,
) -> f64 {
    let vs = nn::VarStore::new(device);
    // GNN with 8 input features
    let policy_net = GraphPolicyNet::new(&vs, 8, hidden_dims, dropout);

    let mut opt = nn::Adam::default().build(&vs, lr).unwrap();

    let batch_size = 32;
    let num_batches = data.len() / batch_size;
    let mut rng = StdRng::seed_from_u64(seed);

    for epoch in 0..epochs {
        let mut epoch_loss = 0.0;
        let mut indices: Vec<usize> = (0..data.len()).collect();
        indices.shuffle(&mut rng);

        for batch_idx in 0..num_batches {
            let batch_indices = &indices[batch_idx * batch_size..(batch_idx + 1) * batch_size];

            let features: Vec<Tensor> = batch_indices
                .iter()
                .map(|&i| data[i].features.shallow_clone())
                .collect();
            let targets: Vec<i64> = batch_indices.iter().map(|&i| data[i].best_pos).collect();

            let batch_features = Tensor::stack(&features, 0);
            let batch_targets = Tensor::from_slice(&targets);

            let logits = policy_net.forward(&batch_features, true);
            let loss = logits.cross_entropy_for_logits(&batch_targets);

            opt.backward_step(&loss);
            epoch_loss += f64::try_from(&loss).unwrap();
        }

        let avg_loss = epoch_loss / num_batches as f64;
        if (epoch + 1) % 10 == 0 || epoch == 0 {
            println!("   Epoch {}/{}: loss = {:.4}", epoch + 1, epochs, avg_loss);
        }
    }

    evaluate_gnn_policy(&policy_net, eval_games, seed)
}

/// Evaluate GAT policy
fn evaluate_gat_policy(policy: &GATPolicyNet, num_games: usize, seed: u64, name: &str) -> f64 {
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

            // Use 47-channel features for GAT
            let features = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            let features_batch = features.unsqueeze(0);
            let logits = policy.forward(&features_batch, false);

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

/// Evaluate GNN policy
fn evaluate_gnn_policy(policy: &GraphPolicyNet, num_games: usize, seed: u64) -> f64 {
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

            let features = convert_plateau_for_gnn_with_tile(&plateau, &tile, turn, 19);
            let features_batch = features.unsqueeze(0);
            let logits = policy.forward(&features_batch, false);

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
    println!("   GNN (8ch) evaluation: {:.2} pts average over {} games", avg, num_games);
    avg
}

// Silent versions for stability analysis
fn train_and_eval_silent_gat(data: &[GATSample], hidden_dims: &[i64], num_heads: usize, dropout: f64, epochs: usize, lr: f64, eval_games: usize, seed: u64, device: Device) -> f64 {
    let vs = nn::VarStore::new(device);
    let policy_net = GATPolicyNet::new(&vs, 47, hidden_dims, num_heads, dropout);
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
            let features = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
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

fn train_and_eval_silent_gnn(data: &[GNNSample], hidden_dims: &[i64], dropout: f64, epochs: usize, lr: f64, eval_games: usize, seed: u64, device: Device) -> f64 {
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

fn evaluate_random(num_games: usize, seed: u64) -> f64 {
    let mut total_score = 0;
    let mut rng = StdRng::seed_from_u64(seed + 20000);

    for _ in 0..num_games {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();

        for _turn in 0..19 {
            let available_tiles = get_available_tiles(&deck);
            if available_tiles.is_empty() { break; }
            let tile = *available_tiles.choose(&mut rng).unwrap();
            let available_positions: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if available_positions.is_empty() { break; }
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

fn evaluate_greedy(num_games: usize, seed: u64) -> f64 {
    let mut total_score = 0;
    let mut rng = StdRng::seed_from_u64(seed + 30000);

    for _ in 0..num_games {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();

        for _turn in 0..19 {
            let available_tiles = get_available_tiles(&deck);
            if available_tiles.is_empty() { break; }
            let tile = *available_tiles.choose(&mut rng).unwrap();
            let available_positions: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if available_positions.is_empty() { break; }
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
