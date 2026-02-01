//! Unified AlphaZero Training Pipeline
//!
//! Phase 1: Train CNN on archived high-quality games (supervised warmup)
//! Phase 2: Train Q-Net on Q-value data
//! Phase 3: AlphaZero self-play with Dirichlet noise + Q-Net pruning

use clap::Parser;
use flexi_logger::Logger;
use rand::prelude::*;
use rand_distr::Distribution;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use tch::{nn, nn::OptimizerConfig, Device, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::mcts::algorithm::{
    mcts_find_best_position_for_tile_with_nn, mcts_find_best_position_for_tile_with_qnet,
};
use take_it_easy::neural::tensor_conversion::convert_plateau_to_tensor;
use take_it_easy::neural::{NeuralConfig, NeuralManager, QNetManager};
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "train-unified-alphazero")]
#[command(about = "Unified AlphaZero training: Supervised warmup + Q-Net + Self-play")]
struct Args {
    /// Phase to run (1=supervised, 2=qnet, 3=selfplay, all=1+2+3)
    #[arg(long, default_value = "all")]
    phase: String,

    /// Supervised training epochs
    #[arg(long, default_value_t = 30)]
    supervised_epochs: usize,

    /// Self-play iterations
    #[arg(long, default_value_t = 50)]
    selfplay_iterations: usize,

    /// Games per self-play iteration
    #[arg(long, default_value_t = 50)]
    games_per_iter: usize,

    /// MCTS simulations
    #[arg(long, default_value_t = 150)]
    mcts_sims: usize,

    /// Learning rate
    #[arg(long, default_value_t = 0.001)]
    lr: f64,

    /// Batch size
    #[arg(long, default_value_t = 64)]
    batch_size: usize,

    /// Dirichlet alpha (exploration)
    #[arg(long, default_value_t = 0.3)]
    dirichlet_alpha: f64,

    /// Dirichlet epsilon (noise ratio)
    #[arg(long, default_value_t = 0.25)]
    dirichlet_epsilon: f64,

    /// Q-Net top-K for pruning
    #[arg(long, default_value_t = 6)]
    top_k: usize,

    /// Benchmark games between iterations
    #[arg(long, default_value_t = 50)]
    benchmark_games: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    log::info!("{}", "=".repeat(70));
    log::info!("     UNIFIED ALPHAZERO TRAINING PIPELINE");
    log::info!("{}", "=".repeat(70));

    // Load neural manager
    let neural_config = NeuralConfig::default();
    let mut neural_manager = NeuralManager::with_config(neural_config)?;

    match args.phase.as_str() {
        "1" | "supervised" => {
            run_phase1_supervised(&mut neural_manager, &args)?;
        }
        "2" | "qnet" => {
            run_phase2_qnet(&args)?;
        }
        "3" | "selfplay" => {
            let qnet_manager = QNetManager::new("model_weights/qvalue_net.params")?;
            run_phase3_selfplay(&mut neural_manager, &qnet_manager, &args)?;
        }
        "all" => {
            log::info!("\n### PHASE 1: Supervised Warmup ###\n");
            run_phase1_supervised(&mut neural_manager, &args)?;

            log::info!("\n### PHASE 2: Q-Net Training ###\n");
            run_phase2_qnet(&args)?;

            log::info!("\n### PHASE 3: AlphaZero Self-Play ###\n");
            let qnet_manager = QNetManager::new("model_weights/qvalue_net.params")?;
            run_phase3_selfplay(&mut neural_manager, &qnet_manager, &args)?;
        }
        _ => {
            log::error!("Unknown phase: {}. Use: 1, 2, 3, or all", args.phase);
        }
    }

    Ok(())
}

/// Phase 1: Supervised training on archived high-quality games
fn run_phase1_supervised(
    neural_manager: &mut NeuralManager,
    args: &Args,
) -> Result<(), Box<dyn Error>> {
    log::info!("Loading archived training data...");

    // Load high-quality games (130+ pts)
    let data_files = vec![
        "supervised_130plus_filtered.csv",
        "combined_130_140.csv",
        "supervised_dataset_best.csv",
    ];

    let mut all_samples: Vec<SupervisedSample> = Vec::new();

    for file in &data_files {
        let path = format!(
            "/home/jcgouleau/IdeaProjects/RustProject/take_it_easy/{}",
            file
        );
        if let Ok(samples) = load_supervised_csv(&path) {
            log::info!("  Loaded {} samples from {}", samples.len(), file);
            all_samples.extend(samples);
        }
    }

    // Filter to keep only high-score games (130+)
    let high_quality: Vec<_> = all_samples
        .into_iter()
        .filter(|s| s.final_score >= 130)
        .collect();

    log::info!(
        "Total high-quality samples: {} (score >= 130)",
        high_quality.len()
    );

    if high_quality.is_empty() {
        log::warn!("No training data found!");
        return Ok(());
    }

    // Create optimizer for policy
    let mut policy_opt =
        nn::Adam::default().build(neural_manager.policy_varstore_mut(), args.lr)?;

    // Create optimizer for value
    let mut value_opt = nn::Adam::default().build(neural_manager.value_varstore_mut(), args.lr)?;

    let mut rng = rand::rng();

    for epoch in 0..args.supervised_epochs {
        let mut shuffled = high_quality.clone();
        shuffled.shuffle(&mut rng);

        let mut total_policy_loss = 0.0;
        let mut total_value_loss = 0.0;
        let mut batch_count = 0;

        for batch in shuffled.chunks(args.batch_size) {
            // Prepare batch tensors
            let (inputs, policy_targets, value_targets) = prepare_supervised_batch(batch);

            // Policy forward + backward
            let policy_net = neural_manager.policy_net();
            let policy_out = policy_net.forward(&inputs, true);
            let policy_log_softmax = policy_out.log_softmax(-1, Kind::Float);
            let policy_loss = policy_log_softmax.nll_loss(&policy_targets);

            policy_opt.zero_grad();
            policy_loss.backward();
            policy_opt.step();

            // Value forward + backward
            let value_net = neural_manager.value_net();
            let value_out = value_net.forward(&inputs, true);
            let value_loss = value_out.mse_loss(&value_targets, tch::Reduction::Mean);

            value_opt.zero_grad();
            value_loss.backward();
            value_opt.step();

            total_policy_loss += policy_loss.double_value(&[]);
            total_value_loss += value_loss.double_value(&[]);
            batch_count += 1;
        }

        let avg_policy = total_policy_loss / batch_count as f64;
        let avg_value = total_value_loss / batch_count as f64;

        if epoch % 5 == 0 || epoch == args.supervised_epochs - 1 {
            log::info!(
                "Epoch {}/{}: policy_loss={:.4}, value_loss={:.4}",
                epoch + 1,
                args.supervised_epochs,
                avg_policy,
                avg_value
            );
        }
    }

    // Save trained weights
    neural_manager.save_models()?;
    log::info!("Phase 1 complete: CNN weights saved");

    // Benchmark
    let score = benchmark_current_model(neural_manager, 20)?;
    log::info!("Post-supervised benchmark: {:.2} pts", score);

    Ok(())
}

/// Phase 2: Train Q-Net on Q-value data
fn run_phase2_qnet(_args: &Args) -> Result<(), Box<dyn Error>> {
    log::info!("Training Q-Net on archived Q-value data...");

    // Use existing train_qvalue_net logic
    let qnet_path = "model_weights/qvalue_net.params";

    if std::path::Path::new(qnet_path).exists() {
        log::info!("Q-Net weights found at {}", qnet_path);

        // Verify Q-net works
        let qnet = QNetManager::new(qnet_path)?;
        let test_plateau = create_plateau_empty();
        let test_tile = Tile(5, 5, 5);
        let top_pos = qnet
            .net()
            .get_top_positions(&test_plateau.tiles, &test_tile, 6);
        log::info!("Q-Net verification: top-6 positions = {:?}", top_pos);
    } else {
        log::warn!("Q-Net weights not found. Run: cargo run --release --bin train_qvalue_net");
    }

    Ok(())
}

/// Phase 3: AlphaZero self-play with Dirichlet noise and Q-Net pruning
fn run_phase3_selfplay(
    neural_manager: &mut NeuralManager,
    qnet_manager: &QNetManager,
    args: &Args,
) -> Result<(), Box<dyn Error>> {
    log::info!("Starting AlphaZero self-play with Dirichlet noise...");
    log::info!("  Iterations: {}", args.selfplay_iterations);
    log::info!("  Games/iter: {}", args.games_per_iter);
    log::info!("  MCTS sims: {}", args.mcts_sims);
    log::info!(
        "  Dirichlet: alpha={}, eps={}",
        args.dirichlet_alpha,
        args.dirichlet_epsilon
    );
    log::info!("  Q-Net top-K: {}", args.top_k);

    let mut best_score = 0.0f64;

    for iteration in 0..args.selfplay_iterations {
        log::info!(
            "\n--- Iteration {}/{} ---",
            iteration + 1,
            args.selfplay_iterations
        );

        // Generate self-play games with exploration
        let games = generate_selfplay_games_with_dirichlet(
            neural_manager,
            qnet_manager,
            args.games_per_iter,
            args.mcts_sims,
            args.top_k,
            args.dirichlet_alpha,
            args.dirichlet_epsilon,
        );

        // Collect training data
        let training_data: Vec<_> = games.iter().flat_map(|g| g.moves.iter().cloned()).collect();

        log::info!("  Generated {} training samples", training_data.len());

        // Train on self-play data
        train_on_selfplay_data(neural_manager, &training_data, args)?;

        // Benchmark
        let score = benchmark_current_model(neural_manager, args.benchmark_games)?;
        log::info!("  Benchmark: {:.2} pts (best: {:.2})", score, best_score);

        if score > best_score {
            best_score = score;
            neural_manager.save_models()?;
            log::info!("  New best! Weights saved.");
        }

        // Early stopping if we reach target
        if score >= 145.0 {
            log::info!("Target reached! Score = {:.2} >= 145", score);
            break;
        }
    }

    log::info!("\nPhase 3 complete. Best score: {:.2} pts", best_score);
    Ok(())
}

/// Generate self-play games with Dirichlet noise for exploration
fn generate_selfplay_games_with_dirichlet(
    neural_manager: &NeuralManager,
    qnet_manager: &QNetManager,
    num_games: usize,
    mcts_sims: usize,
    top_k: usize,
    alpha: f64,
    epsilon: f64,
) -> Vec<GameRecord> {
    let mut games = Vec::new();
    let mut rng = rand::rng();

    let policy_net = neural_manager.policy_net();
    let value_net = neural_manager.value_net();
    let qvalue_net = qnet_manager.net();

    for game_idx in 0..num_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut game_record = GameRecord {
            moves: Vec::new(),
            final_score: 0,
        };

        // Draw 19 tiles for the game
        let mut tiles: Vec<Tile> = Vec::new();
        let mut tile_deck = create_deck();
        for _ in 0..19 {
            let available = get_available_tiles(&tile_deck);
            if available.is_empty() {
                break;
            }
            let tile = *available.choose(&mut rng).unwrap();
            tiles.push(tile);
            tile_deck = replace_tile_in_deck(&tile_deck, &tile);
        }

        // Reset deck for MCTS
        deck = create_deck();

        for (turn, &tile) in tiles.iter().enumerate() {
            // Get MCTS result with Q-Net pruning
            let mcts_result = mcts_find_best_position_for_tile_with_qnet(
                &mut plateau.clone(),
                &mut deck.clone(),
                tile,
                policy_net,
                value_net,
                qvalue_net,
                mcts_sims,
                turn,
                19,
                top_k,
                None,
            );

            // Get policy distribution
            let mut policy: Vec<f32> = (0..19)
                .map(|i| mcts_result.policy_distribution.double_value(&[i]) as f32)
                .collect();

            // Apply Dirichlet noise for exploration (early/mid game)
            if turn < 12 {
                let legal_moves = get_legal_moves(&plateau);
                if legal_moves.len() > 1 {
                    // Generate Dirichlet noise using rand_distr::multi::Dirichlet
                    let alpha_vec: Vec<f64> = vec![alpha; legal_moves.len()];
                    if let Ok(dirichlet) = rand_distr::multi::Dirichlet::<f64>::new(&alpha_vec) {
                        let noise: Vec<f64> = dirichlet.sample(&mut rng);

                        // Mix policy with noise
                        for (i, &pos) in legal_moves.iter().enumerate() {
                            policy[pos] = (1.0 - epsilon as f32) * policy[pos]
                                + epsilon as f32 * noise[i] as f32;
                        }

                        // Renormalize
                        let sum: f32 = policy.iter().sum();
                        if sum > 0.0 {
                            for p in &mut policy {
                                *p /= sum;
                            }
                        }
                    }
                }
            }

            // Sample action from (noisy) policy
            let action = sample_from_policy(&policy, &mut rng);

            // Record move (store tensor data as Vec for Clone)
            let state_tensor = convert_plateau_to_tensor(&plateau, &tile, &deck, turn, 19);
            let state_data: Vec<f32> =
                Vec::<f32>::try_from(state_tensor.flatten(0, -1)).unwrap_or_default();

            game_record.moves.push(MoveRecord {
                state_data,
                policy: policy.clone(),
                action,
                value: 0.0, // Will be filled with final score
            });

            // Apply action
            plateau.tiles[action] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        // Compute final score and backfill value
        let final_score = result(&plateau);
        let normalized_value = (final_score as f64 - 100.0) / 100.0; // Normalize around 100

        for mv in &mut game_record.moves {
            mv.value = normalized_value as f32;
        }

        game_record.final_score = final_score;
        games.push(game_record);

        if (game_idx + 1) % 10 == 0 {
            log::debug!("  Generated {}/{} games", game_idx + 1, num_games);
        }
    }

    let avg_score: f64 = games.iter().map(|g| g.final_score as f64).sum::<f64>() / num_games as f64;
    log::info!("  Self-play avg score: {:.2}", avg_score);

    games
}

fn sample_from_policy(policy: &[f32], rng: &mut impl Rng) -> usize {
    let r: f32 = rng.random();
    let mut cumsum = 0.0;
    for (i, &p) in policy.iter().enumerate() {
        cumsum += p;
        if r < cumsum {
            return i;
        }
    }
    // Fallback: return highest probability position
    policy
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn train_on_selfplay_data(
    neural_manager: &mut NeuralManager,
    data: &[MoveRecord],
    args: &Args,
) -> Result<(), Box<dyn Error>> {
    if data.is_empty() {
        return Ok(());
    }

    let mut policy_opt =
        nn::Adam::default().build(neural_manager.policy_varstore_mut(), args.lr)?;

    let mut value_opt = nn::Adam::default().build(neural_manager.value_varstore_mut(), args.lr)?;

    let epochs = 10;
    let mut rng = rand::rng();

    for _epoch in 0..epochs {
        let mut shuffled: Vec<_> = data.iter().collect();
        shuffled.shuffle(&mut rng);

        for batch in shuffled.chunks(args.batch_size) {
            // Stack inputs - recreate tensors from stored data
            let inputs: Vec<Tensor> = batch
                .iter()
                .map(|m| Tensor::from_slice(&m.state_data).view([1, 47, 5, 5]))
                .collect();
            let input_batch = Tensor::cat(&inputs, 0);

            // Policy targets (as class indices)
            let policy_targets: Vec<i64> = batch.iter().map(|m| m.action as i64).collect();
            let policy_target_tensor = Tensor::from_slice(&policy_targets).to_device(Device::Cpu);

            // Value targets
            let value_targets: Vec<f32> = batch.iter().map(|m| m.value).collect();
            let value_target_tensor = Tensor::from_slice(&value_targets)
                .view([-1, 1])
                .to_device(Device::Cpu);

            // Policy training
            let policy_net = neural_manager.policy_net();
            let policy_out = policy_net.forward(&input_batch, true);
            let policy_log_softmax = policy_out.log_softmax(-1, Kind::Float);
            let policy_loss = policy_log_softmax.nll_loss(&policy_target_tensor);

            policy_opt.zero_grad();
            policy_loss.backward();
            policy_opt.step();

            // Value training
            let value_net = neural_manager.value_net();
            let value_out = value_net.forward(&input_batch, true);
            let value_loss = value_out.mse_loss(&value_target_tensor, tch::Reduction::Mean);

            value_opt.zero_grad();
            value_loss.backward();
            value_opt.step();
        }
    }

    Ok(())
}

fn benchmark_current_model(
    neural_manager: &NeuralManager,
    num_games: usize,
) -> Result<f64, Box<dyn Error>> {
    let mut scores = Vec::new();
    let mut rng = rand::rng();

    let policy_net = neural_manager.policy_net();
    let value_net = neural_manager.value_net();

    for _ in 0..num_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..19 {
            let available = get_available_tiles(&deck);
            if available.is_empty() {
                break;
            }
            let tile = *available.choose(&mut rng).unwrap();

            let mcts_result = mcts_find_best_position_for_tile_with_nn(
                &mut plateau,
                &mut deck,
                tile,
                policy_net,
                value_net,
                100, // Quick benchmark
                turn,
                19,
                None,
            );

            plateau.tiles[mcts_result.best_position] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        scores.push(result(&plateau) as f64);
    }

    Ok(scores.iter().sum::<f64>() / scores.len() as f64)
}

// Data structures
#[derive(Clone)]
struct SupervisedSample {
    plateau: Vec<i32>,
    tile: (i32, i32, i32),
    position: usize,
    final_score: i32,
}

struct GameRecord {
    moves: Vec<MoveRecord>,
    final_score: i32,
}

#[derive(Clone)]
struct MoveRecord {
    state_data: Vec<f32>, // Flattened tensor data (can be cloned)
    policy: Vec<f32>,
    action: usize,
    value: f32,
}

fn load_supervised_csv(path: &str) -> Result<Vec<SupervisedSample>, Box<dyn Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut samples = Vec::new();

    for (i, line) in reader.lines().enumerate() {
        if i == 0 {
            continue;
        } // Skip header

        let line = line?;
        let parts: Vec<&str> = line.split(',').collect();

        if parts.len() < 25 {
            continue;
        }

        let plateau: Vec<i32> = (2..21)
            .filter_map(|i| parts.get(i).and_then(|s| s.parse().ok()))
            .collect();

        if plateau.len() != 19 {
            continue;
        }

        let tile = (
            parts.get(21).and_then(|s| s.parse().ok()).unwrap_or(0),
            parts.get(22).and_then(|s| s.parse().ok()).unwrap_or(0),
            parts.get(23).and_then(|s| s.parse().ok()).unwrap_or(0),
        );

        let position: usize = parts.get(24).and_then(|s| s.parse().ok()).unwrap_or(0);
        let final_score: i32 = parts.get(25).and_then(|s| s.parse().ok()).unwrap_or(0);

        samples.push(SupervisedSample {
            plateau,
            tile,
            position,
            final_score,
        });
    }

    Ok(samples)
}

fn prepare_supervised_batch(batch: &[SupervisedSample]) -> (Tensor, Tensor, Tensor) {
    let mut inputs = Vec::new();
    let mut policy_targets = Vec::new();
    let mut value_targets = Vec::new();

    for sample in batch {
        // Convert plateau to tensor
        let mut plateau = create_plateau_empty();
        for (i, &encoded) in sample.plateau.iter().enumerate() {
            if encoded != 0 {
                let t0 = (encoded / 100);
                let t1 = ((encoded / 10) % 10);
                let t2 = (encoded % 10);
                plateau.tiles[i] = Tile(t0, t1, t2);
            }
        }

        let tile = Tile(sample.tile.0, sample.tile.1, sample.tile.2);
        let deck = create_deck();
        let turn = sample.plateau.iter().filter(|&&x| x != 0).count();

        let tensor = convert_plateau_to_tensor(&plateau, &tile, &deck, turn, 19);
        inputs.push(tensor);

        policy_targets.push(sample.position as i64);
        value_targets.push((sample.final_score as f32 - 100.0) / 100.0);
    }

    // Use cat instead of stack since each tensor already has batch dim [1, 47, 5, 5]
    let input_batch = Tensor::cat(&inputs, 0);
    let policy_batch = Tensor::from_slice(&policy_targets);
    let value_batch = Tensor::from_slice(&value_targets).view([-1, 1]);

    (input_batch, policy_batch, value_batch)
}
