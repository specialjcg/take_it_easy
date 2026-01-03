//! AlphaGo Zero Style Iterative Training with UCT MCTS
//!
//! Implements iterative self-play training loop using UCT MCTS (+101% performance):
//! 1. Generate games using UCT MCTS (149 pts vs 74 pts batch)
//! 2. Train networks on high-quality UCT self-play data
//! 3. Benchmark to measure convergence
//! 4. Repeat until networks converge
//!
//! UCT MCTS concentrates exploration on promising positions for better estimates.

use clap::Parser;
use flexi_logger::Logger;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::prelude::IndexedRandom;
use rand_distr::{Distribution, Gamma};
use std::fs::File;
use std::io::Write;
use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_uct;
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::tensor_conversion::convert_plateau_to_tensor;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use take_it_easy::scoring::scoring::result;
use tch::{Device, Tensor};

#[derive(Parser, Debug)]
#[command(name = "alphago-zero-trainer")]
#[command(about = "AlphaGo Zero style iterative training with convergence")]
struct Args {
    /// Number of training iterations
    #[arg(long, default_value_t = 50)]
    iterations: usize,

    /// Games per iteration (self-play)
    #[arg(long, default_value_t = 20)]
    games_per_iter: usize,

    /// MCTS simulations for self-play
    #[arg(long, default_value_t = 150)]
    mcts_simulations: usize,

    /// Training epochs per iteration
    #[arg(long, default_value_t = 10)]
    epochs_per_iter: usize,

    /// Learning rate
    #[arg(long, default_value_t = 0.01)]
    learning_rate: f64,

    /// Batch size for training
    #[arg(long, default_value_t = 32)]
    batch_size: usize,

    /// Benchmark games (for convergence check)
    #[arg(long, default_value_t = 100)]
    benchmark_games: usize,

    /// Convergence threshold (stop if score improvement < this)
    #[arg(long, default_value_t = 2.0)]
    convergence_threshold: f64,

    /// Random seed
    #[arg(long, default_value_t = 2025)]
    seed: u64,

    /// Neural network architecture
    #[arg(long, default_value = "CNN")]
    nn_architecture: String,

    /// Start fresh (don't load existing weights)
    #[arg(long, default_value_t = false)]
    fresh_start: bool,

    /// Use Q-value based policy targets instead of visit counts
    /// This trains the policy network on rollout quality rather than visit frequency
    #[arg(long, default_value_t = false)]
    use_q_value_targets: bool,

    /// Save generated training data to disk (format: {path}_cnn_states.pt, etc.)
    /// Use this to generate quality data for supervised learning
    #[arg(long)]
    save_data_path: Option<String>,

    /// Output file for training history
    #[arg(long, default_value = "training_history.csv")]
    output: String,
}

struct TrainingExample {
    state: Tensor,
    policy_target: Vec<f32>,  // FIXED: Use visit distribution instead of argmax
    value_target: f32,
}

/// Save training data to disk in format compatible with load_game_data()
fn save_training_data(
    data: &[TrainingExample],
    path: &str,
    arch: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if data.is_empty() {
        log::warn!("No data to save");
        return Ok(());
    }

    let arch_suffix = match arch {
        "CNN" => "_cnn",
        "GNN" => "_gnn",
        _ => "_cnn",
    };
    let prefixed_path = format!("{}{}", path, arch_suffix);

    log::info!("💾 Saving {} training examples to {}...", data.len(), prefixed_path);

    // Stack all state tensors into a single tensor [N, C, H, W]
    let states: Vec<Tensor> = data.iter().map(|ex| ex.state.shallow_clone()).collect();
    let states_tensor = Tensor::stack(&states, 0);

    // Create positions tensor from policy targets (argmax of policy distribution)
    let positions: Vec<i64> = data
        .iter()
        .map(|ex| {
            ex.policy_target
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(idx, _)| idx as i64)
                .unwrap_or(0)
        })
        .collect();
    let positions_tensor = Tensor::from_slice(&positions);

    // Create subscores tensor from value targets
    let subscores: Vec<f64> = data.iter().map(|ex| ex.value_target as f64).collect();
    let subscores_tensor = Tensor::from_slice(&subscores);

    // Save tensors
    states_tensor.save(&format!("{}_states.pt", prefixed_path))?;
    positions_tensor.save(&format!("{}_positions.pt", prefixed_path))?;
    subscores_tensor.save(&format!("{}_subscores.pt", prefixed_path))?;

    log::info!("✅ Training data saved successfully");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    let args = Args::parse();

    log::info!("🚀 AlphaGo Zero Style Training");
    log::info!("   Iterations: {}", args.iterations);
    log::info!("   Games/iter: {}, MCTS sims: {}", args.games_per_iter, args.mcts_simulations);
    log::info!("   Epochs/iter: {}, LR: {}", args.epochs_per_iter, args.learning_rate);
    log::info!("   Benchmark games: {}", args.benchmark_games);
    log::info!("   Convergence threshold: {:.2} pts", args.convergence_threshold);

    // Parse architecture
    let nn_arch = match args.nn_architecture.to_uppercase().as_str() {
        "CNN" => NNArchitecture::Cnn,
        "GNN" => NNArchitecture::Gnn,
        _ => return Err(format!("Invalid architecture: {}", args.nn_architecture).into()),
    };

    // Initialize neural network
    let neural_config = NeuralConfig {
        input_dim: (9, 5, 5),
        nn_architecture: nn_arch,
        policy_lr: args.learning_rate,
        value_lr: args.learning_rate,
        ..Default::default()
    };

    // Optionally start fresh
    let model_path_backup = if args.fresh_start {
        let backup = std::env::var("MODEL_PATH").ok();
        std::env::set_var("MODEL_PATH", "/nonexistent");
        log::info!("   Starting with fresh weights (no loading)");
        Some(backup)
    } else {
        log::info!("   Loading existing weights if available");
        None
    };

    let mut manager = NeuralManager::with_config(neural_config)?;

    // Restore env
    if let Some(Some(path)) = model_path_backup {
        std::env::set_var("MODEL_PATH", path);
    }

    // Initialize training history file
    let mut history_file = File::create(&args.output)?;
    writeln!(history_file, "iteration,policy_loss,value_loss,benchmark_score_mean,benchmark_score_std")?;

    let mut previous_score = 0.0;
    let device = Device::Cpu;

    // Main training loop
    for iteration in 0..args.iterations {
        log::info!("\n{}", "=".repeat(60));
        log::info!("📊 Iteration {}/{}", iteration + 1, args.iterations);
        log::info!("{}", "=".repeat(60));

        // Step 1: Self-play to generate training data
        log::info!("\n🎮 Phase 1: Self-play ({} games)", args.games_per_iter);
        let training_data = generate_self_play_games(
            &manager,
            args.games_per_iter,
            args.mcts_simulations,
            args.seed + iteration as u64,
            args.use_q_value_targets,
        )?;

        log::info!("   Generated {} training examples", training_data.len());

        // Save training data if requested
        if let Some(ref save_path) = args.save_data_path {
            let iteration_path = format!("{}_{}", save_path, iteration);
            save_training_data(&training_data, &iteration_path, &args.nn_architecture)?;
        }

        // Step 2: Train on self-play data
        log::info!("\n🏋️ Phase 2: Training ({} epochs)", args.epochs_per_iter);
        let (avg_policy_loss, avg_value_loss) = train_on_data(
            &training_data,
            &mut manager,
            args.epochs_per_iter,
            args.batch_size,
            device,
        )?;

        log::info!("   Final losses: policy={:.4}, value={:.4}", avg_policy_loss, avg_value_loss);

        // Step 3: Benchmark to measure progress
        log::info!("\n📈 Phase 3: Benchmark ({} games)", args.benchmark_games);
        let (benchmark_mean, benchmark_std) = benchmark_performance(
            &manager,
            args.benchmark_games,
            args.mcts_simulations,
            args.seed + 1000 + iteration as u64,
        )?;

        log::info!("   Score: {:.2} ± {:.2}", benchmark_mean, benchmark_std);

        // Record history
        writeln!(
            history_file,
            "{},{:.4},{:.4},{:.2},{:.2}",
            iteration + 1,
            avg_policy_loss,
            avg_value_loss,
            benchmark_mean,
            benchmark_std
        )?;
        history_file.flush()?;

        // Step 4: Check convergence
        let improvement = benchmark_mean - previous_score;
        log::info!("\n🎯 Progress Check:");
        log::info!("   Previous score: {:.2}", previous_score);
        log::info!("   Current score:  {:.2}", benchmark_mean);
        log::info!("   Improvement:    {:.2}", improvement);

        // Only converge if improvement is VERY SMALL (not just less than threshold)
        // With high threshold, we want to continue training
        if iteration > 0 && args.convergence_threshold < 100.0 && improvement.abs() < args.convergence_threshold {
            log::info!("\n✅ CONVERGED: Improvement < {:.2} pts", args.convergence_threshold);
            log::info!("   Training complete at iteration {}", iteration + 1);
            break;
        } else if iteration > 0 {
            log::info!("   Continuing training (threshold: {:.2} pts)", args.convergence_threshold);
        }

        previous_score = benchmark_mean;

        // Step 5: Save checkpoint
        log::info!("\n💾 Saving checkpoint...");
        manager.save_models()
            .expect("Failed to save model weights");
        log::info!("   ✅ Weights saved successfully");
    }

    log::info!("\n{}", "=".repeat(60));
    log::info!("✅ Training Complete");
    log::info!("   History saved to: {}", args.output);
    log::info!("{}", "=".repeat(60));

    Ok(())
}

fn generate_self_play_games(
    manager: &NeuralManager,
    num_games: usize,
    mcts_sims: usize,
    seed: u64,
    use_q_value_targets: bool,
) -> Result<Vec<TrainingExample>, Box<dyn std::error::Error>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut training_data = Vec::new();
    let turns_per_game = 19;

    for game_idx in 0..num_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..turns_per_game {
            let available = get_available_tiles(&deck);
            if available.is_empty() {
                break;
            }

            let chosen_tile = *available.choose(&mut rng).unwrap();
            deck = replace_tile_in_deck(&deck, &chosen_tile);

            // ====================================================================
            // DIRICHLET NOISE - AlphaGo Zero exploration technique (STRENGTHENED)
            // ====================================================================
            // Add Dirichlet noise to encourage exploration during self-play
            // This breaks the circular learning problem where uniform policy
            // leads to uniform MCTS priors, which generate uniform training data
            // Note: Mixing with epsilon=0.5 happens in MCTS function (src/mcts/algorithm.rs:1507)
            let legal_moves = get_legal_moves(&plateau);
            let alpha = 0.15;    // Dirichlet concentration (LOWERED: lower = more peaked/varied distribution)

            // Generate Dirichlet noise for exploration
            // Dirichlet is sampled using Gamma distributions: X_i ~ Gamma(alpha, 1)
            // Then normalize: Y_i = X_i / sum(X_i)
            let gamma = Gamma::new(alpha, 1.0)
                .expect("Failed to create Gamma distribution");
            let mut noise: Vec<f64> = (0..legal_moves.len())
                .map(|_| gamma.sample(&mut rng))
                .collect();
            let sum: f64 = noise.iter().sum();
            for val in &mut noise {
                *val /= sum;
            }

            // Convert noise to exploration_priors (map position -> noise value)
            let mut exploration_priors = vec![0.0; 19]; // 19 positions on plateau
            for (idx, &pos) in legal_moves.iter().enumerate() {
                // AlphaGo Zero formula: P_noisy = (1-ε)*P_policy + ε*noise
                // The actual mixing happens in MCTS with epsilon=0.5 (strengthened)
                exploration_priors[pos] = noise[idx] as f32;  // Store noise for MCTS mixing
            }

            // DEBUG: Log noise statistics for first game, first turn
            if game_idx == 0 && turn == 0 {
                let noise_values: Vec<f64> = legal_moves.iter().map(|&pos| exploration_priors[pos] as f64).collect();
                let max_noise = noise_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let min_noise = noise_values.iter().cloned().fold(f64::INFINITY, f64::min);
                let avg_noise: f64 = noise_values.iter().sum::<f64>() / noise_values.len() as f64;
                log::info!("🔍 DEBUG Dirichlet Noise (game 0, turn 0):");
                log::info!("   Legal moves: {}", legal_moves.len());
                log::info!("   Noise range: [{:.4}, {:.4}], avg: {:.4}", min_noise, max_noise, avg_noise);
                log::info!("   Noise values: {:?}", noise_values.iter().map(|v| format!("{:.3}", v)).collect::<Vec<_>>());
            }

            // Use UCT MCTS with current network to find best position
            // The exploration_priors will be mixed with network policy
            let mcts_result = mcts_find_best_position_for_tile_uct(
                &mut plateau,
                &mut deck,
                chosen_tile,
                manager.policy_net(),
                manager.value_net(),
                mcts_sims,
                turn,
                turns_per_game,
                None, // Use default hyperparameters
                Some(exploration_priors), // Pass Dirichlet noise for exploration
            );

            // Record training example
            let state_tensor = convert_plateau_to_tensor(&plateau, &chosen_tile, &deck, turn, turns_per_game);

            // ====================================================================
            // POLICY TARGET SELECTION - Q-values vs Visit Counts
            // ====================================================================
            let policy_dist: Vec<f32> = if use_q_value_targets && mcts_result.q_value_distribution.is_some() {
                // Use Q-value distribution (already has temperature=0.5 applied in create_q_value_policy_target)
                // This trains policy on rollout QUALITY rather than visit frequency
                mcts_result.q_value_distribution.as_ref().unwrap()
                    .view([-1])
                    .try_into()
                    .expect("Failed to convert Q-value distribution to Vec<f32>")
            } else {
                // Fallback: Use visit counts with temperature sharpening
                let mut policy_dist: Vec<f32> = mcts_result.policy_distribution
                    .view([-1])
                    .try_into()
                    .expect("Failed to convert policy distribution to Vec<f32>");

                // Temperature < 1.0 sharpens the distribution (emphasizes top moves)
                let temperature = 0.5;
                let inv_temp = 1.0 / temperature;
                for p in &mut policy_dist {
                    if *p > 0.0 {
                        *p = p.powf(inv_temp);
                    }
                }

                // Renormalize to ensure it's a valid probability distribution
                let sum: f32 = policy_dist.iter().sum();
                if sum > 0.0 {
                    for p in &mut policy_dist {
                        *p /= sum;
                    }
                }

                policy_dist
            };

            // Store for later (we'll assign value after game ends)
            training_data.push((state_tensor, policy_dist, 0.0));

            // Execute move
            plateau.tiles[mcts_result.best_position] = chosen_tile;
        }

        // Assign value targets based on final score
        let final_score = result(&plateau);
        let normalized_value = (final_score as f32 - 80.0) / 80.0; // Normalize around current performance
        let normalized_value = normalized_value.max(-1.0).min(1.0);

        // Update value targets for all moves in this game
        let game_start = training_data.len() - turns_per_game.min(training_data.len());
        for i in game_start..training_data.len() {
            training_data[i].2 = normalized_value;
        }

        if (game_idx + 1) % 10 == 0 {
            log::info!("   Generated {}/{} games", game_idx + 1, num_games);
        }
    }

    // Convert to TrainingExample structs
    let examples = training_data
        .into_iter()
        .map(|(state, policy, value)| TrainingExample {
            state,
            policy_target: policy,
            value_target: value,
        })
        .collect();

    Ok(examples)
}

fn train_on_data(
    training_data: &[TrainingExample],
    manager: &mut NeuralManager,
    epochs: usize,
    batch_size: usize,
    device: Device,
) -> Result<(f64, f64), Box<dyn std::error::Error>> {
    let mut final_policy_loss = 0.0;
    let mut final_value_loss = 0.0;

    for epoch in 0..epochs {
        let mut epoch_policy_loss = 0.0;
        let mut epoch_value_loss = 0.0;
        let mut batch_count = 0;

        for batch in training_data.chunks(batch_size) {
            // Prepare batch tensors
            let states: Vec<&Tensor> = batch.iter().map(|ex| &ex.state).collect();
            let states_batch = Tensor::cat(&states, 0).to_device(device);

            // FIXED: Policy targets are now distributions, not indices
            let policy_targets_flat: Vec<f32> = batch.iter()
                .flat_map(|ex| ex.policy_target.iter().copied())
                .collect();
            let policy_targets_batch = Tensor::from_slice(&policy_targets_flat)
                .view([batch.len() as i64, 19])  // 19 positions
                .to_device(device);

            let value_targets: Vec<f32> = batch.iter().map(|ex| ex.value_target).collect();
            let value_targets_batch = Tensor::from_slice(&value_targets)
                .view([batch.len() as i64, 1])
                .to_device(device);

            // Train policy network with KL divergence (soft cross-entropy)
            let policy_net = manager.policy_net();
            let policy_pred_logits = policy_net.forward(&states_batch, true);
            let policy_pred_probs = policy_pred_logits.log_softmax(-1, tch::Kind::Float);

            // KL divergence: sum(target * (log(target) - log(pred)))
            // Simplified as cross-entropy when target is a distribution
            let policy_loss = -(policy_targets_batch * policy_pred_probs).sum_dim_intlist(
                [-1].as_slice(),
                false,
                tch::Kind::Float,
            ).mean(tch::Kind::Float);

            let policy_opt = manager.policy_optimizer_mut();
            policy_opt.backward_step(&policy_loss);

            // Train value network
            let value_net = manager.value_net();
            let value_pred = value_net.forward(&states_batch, true);
            let value_loss = value_pred.mse_loss(&value_targets_batch, tch::Reduction::Mean);

            let value_opt = manager.value_optimizer_mut();
            value_opt.backward_step(&value_loss);

            epoch_policy_loss += policy_loss.double_value(&[]);
            epoch_value_loss += value_loss.double_value(&[]);
            batch_count += 1;
        }

        final_policy_loss = epoch_policy_loss / batch_count as f64;
        final_value_loss = epoch_value_loss / batch_count as f64;

        if (epoch + 1) % 5 == 0 || epoch == epochs - 1 {
            log::info!(
                "   Epoch {}/{}: policy_loss={:.4}, value_loss={:.4}",
                epoch + 1,
                epochs,
                final_policy_loss,
                final_value_loss
            );
        }
    }

    Ok((final_policy_loss, final_value_loss))
}

fn benchmark_performance(
    manager: &NeuralManager,
    num_games: usize,
    mcts_sims: usize,
    seed: u64,
) -> Result<(f64, f64), Box<dyn std::error::Error>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut scores = Vec::new();
    let turns_per_game = 19;

    for game_idx in 0..num_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..turns_per_game {
            let available = get_available_tiles(&deck);
            if available.is_empty() {
                break;
            }

            let chosen_tile = *available.choose(&mut rng).unwrap();
            deck = replace_tile_in_deck(&deck, &chosen_tile);

            let mcts_result = mcts_find_best_position_for_tile_uct(
                &mut plateau,
                &mut deck,
                chosen_tile,
                manager.policy_net(),
                manager.value_net(),
                mcts_sims,
                turn,
                turns_per_game,
                None,
                None, // No exploration noise for pure benchmarking
            );

            plateau.tiles[mcts_result.best_position] = chosen_tile;
        }

        let final_score = result(&plateau);
        scores.push(final_score as f64);

        if (game_idx + 1) % 25 == 0 {
            log::info!("   Benchmarked {}/{} games", game_idx + 1, num_games);
        }
    }

    let mean = scores.iter().sum::<f64>() / scores.len() as f64;
    let variance = scores
        .iter()
        .map(|&s| {
            let diff = s - mean;
            diff * diff
        })
        .sum::<f64>()
        / scores.len() as f64;
    let std_dev = variance.sqrt();

    Ok((mean, std_dev))
}
