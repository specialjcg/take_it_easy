//! Supervised Training Pipeline: Elite → Large Dataset (19K) → Benchmark
//!
//! Complete training pipeline:
//! 1. Bootstrap on elite data (456 examples from games >140 pts)
//! 2. Finetune on full dataset (19,000 examples, 1000 games, mean 82.9 pts)
//! 3. Benchmark to validate improvements
//!
//! Starts from existing self-play weights (89.30 pts baseline)
//! Data source: expert_data_mcts_1000games.json (1000 games, MCTS Pure 100 sims)

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use take_it_easy::neural::manager::NNArchitecture;
use tch::{Device, Tensor};
use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use take_it_easy::mcts::hyperparameters::MCTSHyperparameters;
use take_it_easy::scoring::scoring::result;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::prelude::IndexedRandom;

#[derive(Serialize, Deserialize)]
struct ExpertExample {
    state: Vec<f32>,
    policy_target: i64,
    value_target: f32,
}

fn load_expert_data(path: &str) -> Result<Vec<ExpertExample>, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let mut json = String::new();
    file.read_to_string(&mut json)?;
    let examples: Vec<ExpertExample> = serde_json::from_str(&json)?;
    Ok(examples)
}

fn train_on_data(
    manager: &mut NeuralManager,
    examples: &[ExpertExample],
    epochs: usize,
    batch_size: usize,
    phase_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let device = Device::Cpu;

    println!("\n🏋️ Training Phase: {}", phase_name);
    println!("   Examples: {}", examples.len());
    println!("   Epochs: {}", epochs);
    println!("   Batch size: {}\n", batch_size);

    for epoch in 0..epochs {
        let mut epoch_policy_loss = 0.0;
        let mut epoch_value_loss = 0.0;
        let mut batch_count = 0;

        // Shuffle examples
        let mut indices: Vec<usize> = (0..examples.len()).collect();
        for i in 0..indices.len() {
            let j = (i + epoch * 7) % indices.len();
            indices.swap(i, j);
        }

        // Mini-batch training
        for batch_start in (0..examples.len()).step_by(batch_size) {
            let batch_end = (batch_start + batch_size).min(examples.len());
            let batch_indices = &indices[batch_start..batch_end];

            if batch_indices.len() < 8 {
                continue;
            }

            // Prepare batch tensors
            let mut states_vec: Vec<f32> = Vec::new();
            let mut policy_targets_vec: Vec<i64> = Vec::new();
            let mut value_targets_vec: Vec<f32> = Vec::new();

            for &idx in batch_indices {
                let example = &examples[idx];
                states_vec.extend(&example.state);
                policy_targets_vec.push(example.policy_target);
                value_targets_vec.push(example.value_target);
            }

            let states = Tensor::from_slice(&states_vec)
                .view([batch_indices.len() as i64, 8, 5, 5])
                .to_device(device);

            let policy_targets = Tensor::from_slice(&policy_targets_vec)
                .to_device(device);

            let value_targets = Tensor::from_slice(&value_targets_vec)
                .view([batch_indices.len() as i64, 1])
                .to_device(device);

            // Forward pass
            let policy_net = manager.policy_net();
            let value_net = manager.value_net();

            let policy_pred = policy_net.forward(&states, true);
            let value_pred = value_net.forward(&states, true);

            let policy_loss = policy_pred.cross_entropy_for_logits(&policy_targets);
            let value_loss = value_pred.mse_loss(&value_targets, tch::Reduction::Mean);

            // Backward pass
            let policy_opt = manager.policy_optimizer_mut();
            policy_opt.backward_step(&policy_loss);

            let value_opt = manager.value_optimizer_mut();
            value_opt.backward_step(&value_loss);

            // Accumulate losses
            epoch_policy_loss += policy_loss.double_value(&[]);
            epoch_value_loss += value_loss.double_value(&[]);
            batch_count += 1;
        }

        let avg_policy_loss = epoch_policy_loss / batch_count as f64;
        let avg_value_loss = epoch_value_loss / batch_count as f64;

        println!("  Epoch {:2}: policy_loss={:.4}, value_loss={:.4}",
                 epoch + 1, avg_policy_loss, avg_value_loss);
    }

    Ok(())
}

fn benchmark_network(manager: &mut NeuralManager, num_games: usize, simulations: usize) -> Result<f64, Box<dyn std::error::Error>> {
    println!("\n📊 Running benchmark ({} games, {} sims)...", num_games, simulations);

    let mut rng = StdRng::seed_from_u64(2025);
    let mut scores: Vec<i32> = Vec::new();
    let hyperparams = MCTSHyperparameters::default();

    for game_idx in 0..num_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..19 {
            let available = get_available_tiles(&deck);
            if available.is_empty() {
                break;
            }

            let chosen_tile = *available.choose(&mut rng).unwrap();
            deck = replace_tile_in_deck(&deck, &chosen_tile);

            let policy_net = manager.policy_net();
            let value_net = manager.value_net();

            let mcts_result = mcts_find_best_position_for_tile_with_nn(
                &mut plateau,
                &mut deck,
                chosen_tile,
                policy_net,
                value_net,
                simulations,
                turn,
                19,
                Some(&hyperparams),
            );

            plateau.tiles[mcts_result.best_position] = chosen_tile;
        }

        let score = result(&plateau);
        scores.push(score);

        if (game_idx + 1) % 20 == 0 {
            let recent_mean = scores[scores.len().saturating_sub(20)..].iter().sum::<i32>() as f64
                / scores.len().saturating_sub(scores.len() - 20).max(1) as f64;
            println!("  Game {}/{}: Recent avg = {:.2} pts", game_idx + 1, num_games, recent_mean);
        }
    }

    let mean = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
    let std_dev = (scores.iter()
        .map(|&s| (s as f64 - mean).powi(2))
        .sum::<f64>() / scores.len() as f64)
        .sqrt();

    println!("  Final: {:.2} ± {:.2} pts\n", mean, std_dev);
    Ok(mean)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Supervised Training Pipeline: Elite → Large Dataset (19K)\n");
    println!("Starting from self-play weights (89.30 pts baseline)");
    println!("Data: 1000 games MCTS Pure (100 sims, mean 82.9 pts)\n");

    // Load starting weights from self-play
    println!("📂 Loading self-play weights...");
    let weights_path = "model_weights/cnn_current_backup";

    let neural_config = NeuralConfig {
        input_dim: (8, 5, 5),
        nn_architecture: NNArchitecture::CNN,
        policy_lr: 0.01,
        value_lr: 0.01,
        ..Default::default()
    };

    // Load existing weights
    let original_model_path = std::env::var("MODEL_PATH").ok();
    std::env::set_var("MODEL_PATH", weights_path);

    let mut manager = NeuralManager::with_config(neural_config)?;
    println!("   ✅ Loaded weights from {}", weights_path);

    // Restore original MODEL_PATH if it existed
    if let Some(path) = &original_model_path {
        std::env::set_var("MODEL_PATH", path);
    }

    // Baseline benchmark
    let baseline_score = benchmark_network(&mut manager, 100, 150)?;
    println!("📊 Baseline: {:.2} pts\n", baseline_score);

    // Phase 1: Bootstrap on elite data
    println!("{}", "=".repeat(60));
    println!("PHASE 1: Bootstrap on Elite Data (456 examples, >140 pts)");
    println!("{}", "=".repeat(60));

    let elite_data = load_expert_data("expert_data_elite_19k.json")?;
    println!("   Loaded {} examples from elite games", elite_data.len());

    // Train on elite data (larger dataset than before, fewer epochs)
    train_on_data(&mut manager, &elite_data, 30, 32, "Elite Bootstrap (19K)")?;

    // Save checkpoint
    std::env::set_var("MODEL_PATH", "model_elite_bootstrap.ot");
    manager.save_models()?;
    println!("   💾 Saved: model_elite_bootstrap.ot");

    // Benchmark after elite training
    let elite_score = benchmark_network(&mut manager, 100, 150)?;
    println!("📊 After Elite: {:.2} pts ({:+.2} pts, {:+.1}%)\n",
             elite_score, elite_score - baseline_score,
             (elite_score - baseline_score) / baseline_score * 100.0);

    // Phase 2: Finetune on full large dataset
    println!("{}", "=".repeat(60));
    println!("PHASE 2: Finetune on Full Dataset (1000 games, 19K examples)");
    println!("{}", "=".repeat(60));

    let expert_data = load_expert_data("expert_data_mcts_1000games.json")?;
    println!("   Loaded {} examples (mean score 82.9 pts)", expert_data.len());

    // Train on full expert data (large dataset, standard epochs)
    train_on_data(&mut manager, &expert_data, 20, 64, "Full Dataset (19K)")?;

    // Save final supervised model
    std::env::set_var("MODEL_PATH", "model_supervised_final.ot");
    manager.save_models()?;
    println!("   💾 Saved: model_supervised_final.ot");

    // Final benchmark
    let final_score = benchmark_network(&mut manager, 100, 150)?;
    println!("📊 Final Score: {:.2} pts ({:+.2} pts, {:+.1}%)\n",
             final_score, final_score - baseline_score,
             (final_score - baseline_score) / baseline_score * 100.0);

    // Summary
    println!("{}", "=".repeat(60));
    println!("TRAINING SUMMARY");
    println!("{}", "=".repeat(60));
    println!("  Baseline (self-play):        {:.2} pts", baseline_score);
    println!("  After Elite Bootstrap:       {:.2} pts ({:+.2} pts, {:+.1}%)",
             elite_score, elite_score - baseline_score,
             (elite_score - baseline_score) / baseline_score * 100.0);
    println!("  After Full Dataset:          {:.2} pts ({:+.2} pts, {:+.1}%)",
             final_score, final_score - baseline_score,
             (final_score - baseline_score) / baseline_score * 100.0);
    println!("\n📊 Training Data:");
    println!("  Elite: 456 examples (games >140 pts)");
    println!("  Full:  19,000 examples (1000 games, mean 82.9 pts)");
    println!("\n📝 Next Steps:");
    println!("   1. If improvement > 5%: Continue with self-play (alphago_zero_trainer)");
    println!("   2. Set MODEL_PATH=model_supervised_final.ot");
    println!("   3. Run: cargo run --release --bin alphago_zero_trainer");

    Ok(())
}
