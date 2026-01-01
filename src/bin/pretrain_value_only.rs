//! Pretrain ONLY the value network on expert data
//!
//! Strategy: Train value network to predict game scores.
//! MCTS will use: uniform policy + learned value + rollouts.
//! This avoids circular learning (value learns from real game outcomes).

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use take_it_easy::neural::manager::NNArchitecture;
use tch::{Device, Tensor};

#[derive(Serialize, Deserialize)]
struct ExpertExample {
    state: Vec<f32>,
    policy_target: i64,  // Ignored
    value_target: f32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Pretraining VALUE Network Only (No Policy)\n");
    println!("Strategy: MCTS Pure + Learned Value Function\n");

    // Load expert data
    println!("📂 Loading expert data...");
    let mut file = File::open("expert_data_mcts_pure.json")?;
    let mut json = String::new();
    file.read_to_string(&mut json)?;
    let examples: Vec<ExpertExample> = serde_json::from_str(&json)?;
    println!("   Loaded {} examples", examples.len());
    println!("   Ignoring policy targets (will use uniform policy)");

    // Create neural manager
    println!("\n🧠 Initializing neural networks...");
    let neural_config = NeuralConfig {
        input_dim: (9, 5, 5),
        nn_architecture: NNArchitecture::Cnn,
        policy_lr: 0.0,   // No policy training
        value_lr: 0.01,   // Only value training
        ..Default::default()
    };

    // Force fresh network creation
    let model_path_backup = std::env::var("MODEL_PATH").ok();
    std::env::set_var("MODEL_PATH", "/nonexistent");

    let mut manager = NeuralManager::with_config(neural_config)?;

    if let Some(path) = model_path_backup {
        std::env::set_var("MODEL_PATH", path);
    }

    println!("   Created fresh value network (CNN architecture)");
    println!("   Policy network will remain UNTRAINED (uniform)");

    // Training hyperparameters
    let epochs = 50;
    let batch_size = 64;
    let device = Device::Cpu;

    println!("\n🏋️ Training Configuration:");
    println!("   Epochs: {}", epochs);
    println!("   Batch size: {}", batch_size);
    println!("   Value LR: 0.01");
    println!("   Policy: DISABLED (will stay uniform)");
    println!("   Total batches per epoch: {}", examples.len() / batch_size);

    println!("\n🏋️ Starting value network training...\n");

    for epoch in 0..epochs {
        let mut epoch_value_loss = 0.0;
        let mut batch_count = 0;

        // Shuffle examples (deterministic for reproducibility)
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
                continue; // Skip very small batches
            }

            // Prepare batch tensors (only states and value targets)
            let mut states_vec: Vec<f32> = Vec::new();
            let mut value_targets_vec: Vec<f32> = Vec::new();

            for &idx in batch_indices {
                let example = &examples[idx];
                states_vec.extend(&example.state);
                value_targets_vec.push(example.value_target);
            }

            let states = Tensor::from_slice(&states_vec)
                .view([batch_indices.len() as i64, 8, 5, 5])
                .to_device(device);

            let value_targets = Tensor::from_slice(&value_targets_vec)
                .view([batch_indices.len() as i64, 1])
                .to_device(device);

            // Forward pass and loss computation (VALUE ONLY)
            let value_net = manager.value_net();
            let value_pred = value_net.forward(&states, true);
            let value_loss = value_pred.mse_loss(&value_targets, tch::Reduction::Mean);

            // Backward pass (VALUE ONLY)
            let value_opt = manager.value_optimizer_mut();
            value_opt.backward_step(&value_loss);

            // Accumulate losses
            epoch_value_loss += value_loss.double_value(&[]);
            batch_count += 1;
        }

        let avg_value_loss = epoch_value_loss / batch_count as f64;

        println!("  Epoch {:2}: value_loss={:.4}", epoch + 1, avg_value_loss);

        // Save checkpoint every 10 epochs
        if (epoch + 1) % 10 == 0 {
            let old_path = std::env::var("MODEL_PATH").ok();
            let checkpoint_path = format!("model_value_only_epoch{}.ot", epoch + 1);
            std::env::set_var("MODEL_PATH", &checkpoint_path);
            manager.save_models()?;
            if let Some(path) = old_path {
                std::env::set_var("MODEL_PATH", path);
            }
            println!("     → Saved checkpoint: {}", checkpoint_path);
        }
    }

    // Save final model
    let final_path = "model_value_only.ot";
    std::env::set_var("MODEL_PATH", final_path);
    manager.save_models()?;

    println!("\n✅ Value network training complete!");
    println!("   Model saved to: {}", final_path);
    println!("\n📝 Architecture:");
    println!("   Value Network: TRAINED (predicts game scores)");
    println!("   Policy Network: UNTRAINED (stays uniform)");
    println!("\n📝 Next steps:");
    println!("   1. Set MODEL_PATH={}", final_path);
    println!("   2. Run benchmark with value network");
    println!("   3. MCTS will use: uniform policy + trained value + rollouts");
    println!("   4. Expected: Better position evaluation → Better moves");

    Ok(())
}
