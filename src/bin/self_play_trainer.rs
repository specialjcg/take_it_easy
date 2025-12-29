/// Self-Play Trainer for Neural Network Improvement
///
/// Uses iterative self-play to improve neural network weights:
/// 1. Current NN generates games via MCTS
/// 2. Train on those games
/// 3. Repeat
///
/// This is simpler than supervised learning and doesn't require pre-generated expert data.

use clap::Parser;
use flexi_logger::Logger;
use rand::prelude::*;
use rand::rngs::StdRng;
use rand::{SeedableRng, thread_rng};
use std::error::Error;
use tch::{Tensor, Device, Reduction};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::tensor_conversion::convert_plateau_to_tensor;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(
    name = "self-play-trainer",
    about = "Train neural networks via iterative self-play"
)]
struct Args {
    /// Number of self-play iterations
    #[arg(short, long, default_value_t = 10)]
    iterations: usize,

    /// Games per iteration for data generation
    #[arg(short, long, default_value_t = 50)]
    games_per_iter: usize,

    /// MCTS simulations per move during self-play
    #[arg(short = 'm', long, default_value_t = 150)]
    mcts_simulations: usize,

    /// Training epochs per iteration
    #[arg(short, long, default_value_t = 20)]
    epochs: usize,

    /// Batch size for training
    #[arg(short, long, default_value_t = 32)]
    batch_size: usize,

    /// Learning rate
    #[arg(short = 'l', long, default_value_t = 0.0001)]
    learning_rate: f64,

    /// RNG seed
    #[arg(short = 'r', long, default_value_t = 2025)]
    seed: u64,

    /// Benchmark games to evaluate improvement
    #[arg(long, default_value_t = 20)]
    benchmark_games: usize,
}

/// Training example from self-play
struct TrainingExample {
    state_tensor: Tensor,
    best_position: i64,
    final_score: f32,
}

fn main() -> Result<(), Box<dyn Error>> {
    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    let args = Args::parse();

    log::info!("🎓 Self-Play Neural Network Trainer");
    log::info!("Iterations: {}", args.iterations);
    log::info!("Games per iteration: {}", args.games_per_iter);
    log::info!("MCTS simulations: {}", args.mcts_simulations);
    log::info!("Epochs per iteration: {}", args.epochs);
    log::info!("Batch size: {}", args.batch_size);
    log::info!("Learning rate: {}", args.learning_rate);

    // Initialize neural network
    let neural_config = NeuralConfig {
        input_dim: (8, 5, 5),
        nn_architecture: NNArchitecture::CNN,
        policy_lr: args.learning_rate,
        value_lr: args.learning_rate,
        ..Default::default()
    };
    let mut manager = NeuralManager::with_config(neural_config)?;
    log::info!("✅ Neural network initialized\n");

    // Initial benchmark
    log::info!("📊 Initial Benchmark");
    let initial_score = benchmark(&manager, args.benchmark_games, args.seed)?;
    log::info!("Initial average score: {:.2} pts\n", initial_score);

    let mut best_score = initial_score;

    // Self-play training loop
    for iteration in 0..args.iterations {
        log::info!("{}", "=".repeat(70));
        log::info!("🔄 Iteration {}/{}", iteration + 1, args.iterations);
        log::info!("{}", "=".repeat(70));

        // Step 1: Generate self-play data
        log::info!("🎮 Generating {} self-play games...", args.games_per_iter);
        let training_data = generate_self_play_data(
            &manager,
            args.games_per_iter,
            args.mcts_simulations,
            args.seed + iteration as u64,
        )?;
        log::info!(
            "✅ Generated {} training examples (avg score: {:.2})\n",
            training_data.len(),
            training_data.iter().map(|ex| ex.final_score).sum::<f32>()
                / training_data.len() as f32
        );

        // Step 2: Train on self-play data
        log::info!("🏋️ Training for {} epochs...", args.epochs);
        train_iteration(
            &mut manager,
            &training_data,
            args.epochs,
            args.batch_size,
        )?;
        log::info!("✅ Training complete\n");

        // Step 3: Benchmark improvement
        log::info!("📊 Benchmarking iteration {}...", iteration + 1);
        let current_score = benchmark(&manager, args.benchmark_games, args.seed + 10000)?;
        let improvement = current_score - initial_score;
        let relative_improvement = (improvement / initial_score) * 100.0;

        log::info!("Current score: {:.2} pts", current_score);
        log::info!(
            "Improvement: {:+.2} pts ({:+.1}% from initial)",
            improvement,
            relative_improvement
        );

        if current_score > best_score {
            log::info!("🎉 New best score! Saving weights...");
            manager.save_models()?;
            best_score = current_score;
            log::info!("✅ Weights saved\n");
        } else {
            log::info!("⚠️ No improvement over best ({:.2} pts)\n", best_score);
        }
    }

    log::info!("{}", "=".repeat(70));
    log::info!("🎊 Training Complete!");
    log::info!("Initial score: {:.2} pts", initial_score);
    log::info!("Final best score: {:.2} pts", best_score);
    log::info!(
        "Total improvement: {:+.2} pts ({:+.1}%)",
        best_score - initial_score,
        ((best_score - initial_score) / initial_score) * 100.0
    );
    log::info!("{}", "=".repeat(70));

    Ok(())
}

/// Generate self-play training data
fn generate_self_play_data(
    manager: &NeuralManager,
    num_games: usize,
    simulations: usize,
    seed: u64,
) -> Result<Vec<TrainingExample>, Box<dyn Error>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut training_examples = Vec::new();

    let policy_net = manager.policy_net();
    let value_net = manager.value_net();

    for game_idx in 0..num_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        // Sample tile sequence
        let mut tile_order = Vec::new();
        for _ in 0..19 {
            let available = get_available_tiles(&deck);
            if available.is_empty() {
                break;
            }
            let tile = *available.choose(&mut rng).unwrap();
            tile_order.push(tile);
            deck = replace_tile_in_deck(&deck, &tile);
        }

        // Reset and play game
        deck = create_deck();

        for (turn_idx, &chosen_tile) in tile_order.iter().enumerate() {
            let available_tiles = get_available_tiles(&deck);
            if available_tiles.is_empty() || !available_tiles.contains(&chosen_tile) {
                break;
            }

            // Get state before move
            let state_tensor = convert_plateau_to_tensor(&plateau, &chosen_tile, &deck, turn_idx, 19);

            // Run MCTS to find best move
            let mcts_result = mcts_find_best_position_for_tile_with_nn(
                &mut plateau,
                &mut deck,
                chosen_tile,
                policy_net,
                value_net,
                simulations,
                turn_idx,
                19,
                None,
            );

            let best_position = mcts_result.best_position;

            // Make the move
            plateau.tiles[best_position] = chosen_tile;
            deck = replace_tile_in_deck(&deck, &chosen_tile);

            // Store training example (remove batch dimension from state_tensor)
            // convert_plateau_to_tensor returns [1, 8, 5, 5], we need [8, 5, 5]
            let state_tensor_squeezed = state_tensor.squeeze_dim(0);

            training_examples.push(TrainingExample {
                state_tensor: state_tensor_squeezed,
                best_position: best_position as i64,
                final_score: 0.0, // Will be filled after game completes
            });
        }

        // Fill in final scores for all examples from this game
        let final_score = result(&plateau) as f32;
        let game_start_idx = training_examples.len() - tile_order.len();
        for example in training_examples[game_start_idx..].iter_mut() {
            example.final_score = final_score;
        }

        if (game_idx + 1) % 10 == 0 {
            log::info!("  Generated {}/{} games", game_idx + 1, num_games);
        }
    }

    Ok(training_examples)
}

/// Train for one iteration
fn train_iteration(
    manager: &mut NeuralManager,
    training_data: &[TrainingExample],
    epochs: usize,
    batch_size: usize,
) -> Result<(), Box<dyn Error>> {
    let device = Device::Cpu;

    for epoch in 0..epochs {
        let mut total_policy_loss = 0.0;
        let mut total_value_loss = 0.0;
        let mut num_batches = 0;

        // Shuffle training data
        let mut indices: Vec<usize> = (0..training_data.len()).collect();
        indices.shuffle(&mut thread_rng());

        for batch_indices in indices.chunks(batch_size) {
            // Prepare batch
            let batch_states: Vec<Tensor> = batch_indices
                .iter()
                .map(|&i| training_data[i].state_tensor.shallow_clone())
                .collect();
            let batch_states = Tensor::stack(&batch_states, 0).to_device(device);

            let batch_positions: Vec<i64> = batch_indices
                .iter()
                .map(|&i| training_data[i].best_position)
                .collect();
            let batch_positions = Tensor::from_slice(&batch_positions).to_device(device);

            let batch_values: Vec<f32> = batch_indices
                .iter()
                .map(|&i| training_data[i].final_score / 180.0) // Normalize
                .collect();
            let batch_values = Tensor::from_slice(&batch_values)
                .view([-1, 1])
                .to_device(device);

            // Train policy network
            let policy_net = manager.policy_net();
            let policy_pred = policy_net.forward(&batch_states, true);
            let policy_loss = policy_pred.cross_entropy_for_logits(&batch_positions);

            let policy_opt = manager.policy_optimizer_mut();
            policy_opt.backward_step(&policy_loss);
            total_policy_loss += f64::try_from(&policy_loss)?;

            // Train value network
            let value_net = manager.value_net();
            let value_pred = value_net.forward(&batch_states, true);
            let value_loss = value_pred.mse_loss(&batch_values, Reduction::Mean);

            let value_opt = manager.value_optimizer_mut();
            value_opt.backward_step(&value_loss);
            total_value_loss += f64::try_from(&value_loss)?;

            num_batches += 1;
        }

        let avg_policy_loss = total_policy_loss / num_batches as f64;
        let avg_value_loss = total_value_loss / num_batches as f64;

        if (epoch + 1) % 5 == 0 || epoch == 0 {
            log::info!(
                "  Epoch {}/{}: policy_loss={:.4}, value_loss={:.4}",
                epoch + 1,
                epochs,
                avg_policy_loss,
                avg_value_loss
            );
        }
    }

    Ok(())
}

/// Benchmark current network performance
fn benchmark(
    manager: &NeuralManager,
    num_games: usize,
    seed: u64,
) -> Result<f64, Box<dyn Error>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut scores = Vec::new();

    let policy_net = manager.policy_net();
    let value_net = manager.value_net();

    for _ in 0..num_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        // Sample tile sequence
        let mut tile_order = Vec::new();
        for _ in 0..19 {
            let available = get_available_tiles(&deck);
            if available.is_empty() {
                break;
            }
            let tile = *available.choose(&mut rng).unwrap();
            tile_order.push(tile);
            deck = replace_tile_in_deck(&deck, &tile);
        }

        // Reset and play game
        deck = create_deck();

        for (turn_idx, &chosen_tile) in tile_order.iter().enumerate() {
            let available_tiles = get_available_tiles(&deck);
            if available_tiles.is_empty() || !available_tiles.contains(&chosen_tile) {
                break;
            }

            let mcts_result = mcts_find_best_position_for_tile_with_nn(
                &mut plateau,
                &mut deck,
                chosen_tile,
                policy_net,
                value_net,
                150, // Standard simulations for benchmark
                turn_idx,
                19,
                None,
            );

            plateau.tiles[mcts_result.best_position] = chosen_tile;
            deck = replace_tile_in_deck(&deck, &chosen_tile);
        }

        scores.push(result(&plateau));
    }

    let avg_score = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
    Ok(avg_score)
}
