//! Test if Network Can Learn
//!
//! Creates simple synthetic data where we KNOW the answer:
//! - Center positions (9, 4, 14) should have high probability
//! - Edge positions should have low probability
//!
//! If network can learn this, it CAN learn. If not, there's a bug.

use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use tch::{Device, Kind, Tensor};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing if Network Can Learn Simple Patterns\n");

    // Create network from scratch
    println!("📊 Creating fresh network...");
    let neural_config = NeuralConfig {
        input_dim: (9, 5, 5),
        nn_architecture: NNArchitecture::Cnn,
        policy_lr: 0.1, // VERY HIGH LR for fast learning
        value_lr: 0.1,
        ..Default::default()
    };

    // Don't load weights - start fresh
    let model_path_backup = std::env::var("MODEL_PATH").ok();
    std::env::set_var("MODEL_PATH", "/nonexistent"); // Force fresh init

    let mut manager = NeuralManager::with_config(neural_config)?;

    // Restore env
    if let Some(path) = model_path_backup {
        std::env::set_var("MODEL_PATH", path);
    }

    println!("✅ Fresh network created\n");

    // Create synthetic training data
    // Pattern: Center positions (4, 9, 14) should be preferred
    println!("📊 Creating synthetic training data...");
    println!("   Pattern: Center positions preferred (4, 9, 14)");
    println!("   Edge positions discouraged\n");

    let device = Device::Cpu;
    let batch_size = 50;

    // Create batch of empty boards
    let states = Tensor::zeros([batch_size, 8, 5, 5], (Kind::Float, device));

    // Policy targets: center positions have high probability
    let mut policy_targets = vec![9i64; batch_size as usize]; // Position 9 (center)
                                                              // Add some variation
    for i in 0..batch_size as usize {
        policy_targets[i] = match i % 3 {
            0 => 4,  // Center top
            1 => 9,  // Center middle
            _ => 14, // Center bottom
        };
    }
    let policy_targets = Tensor::from_slice(&policy_targets).to_device(device);

    // Value targets: all positive (center is good)
    let value_targets = Tensor::ones([batch_size, 1], (Kind::Float, device)) * 0.5;

    println!("🏋️ Training for 500 epochs on synthetic data...\n");

    for epoch in 0..500 {
        // Train policy
        let policy_net = manager.policy_net();
        let policy_pred = policy_net.forward(&states, true);
        let policy_loss = policy_pred.cross_entropy_for_logits(&policy_targets);

        let policy_opt = manager.policy_optimizer_mut();
        policy_opt.backward_step(&policy_loss);

        // Train value
        let value_net = manager.value_net();
        let value_pred = value_net.forward(&states, true);
        let value_loss = value_pred.mse_loss(&value_targets, tch::Reduction::Mean);

        let value_opt = manager.value_optimizer_mut();
        value_opt.backward_step(&value_loss);

        if (epoch + 1) % 50 == 0 {
            let p_loss: f64 = policy_loss.double_value(&[]);
            let v_loss: f64 = value_loss.double_value(&[]);
            println!(
                "  Epoch {}: policy_loss={:.4}, value_loss={:.4}",
                epoch + 1,
                p_loss,
                v_loss
            );
        }
    }

    println!("\n📊 Testing learned policy...\n");

    // Test on empty board
    let test_state = Tensor::zeros([1, 8, 5, 5], (Kind::Float, device));
    let policy_net = manager.policy_net();
    let policy_pred = policy_net.forward(&test_state, false);
    let policy = policy_pred.softmax(-1, Kind::Float);

    // Extract probabilities
    let probs: Vec<f64> = (0..19).map(|i| policy.double_value(&[0, i])).collect();

    println!("Policy probabilities:");
    for (pos, prob) in probs.iter().enumerate() {
        let marker = if pos == 4 || pos == 9 || pos == 14 {
            " <- CENTER (should be HIGH)"
        } else if pos == 0 || pos == 2 || pos == 7 || pos == 11 || pos == 16 || pos == 18 {
            " (edge, should be low)"
        } else {
            ""
        };
        println!("  Position {:2}: {:.4}{}", pos, prob, marker);
    }

    let center_avg = (probs[4] + probs[9] + probs[14]) / 3.0;
    let edge_avg = (probs[0] + probs[2] + probs[7] + probs[11] + probs[16] + probs[18]) / 6.0;

    println!("\n🎯 Results:");
    println!("   Center positions avg: {:.4}", center_avg);
    println!("   Edge positions avg:   {:.4}", edge_avg);
    println!("   Ratio (center/edge):  {:.2}x", center_avg / edge_avg);

    if center_avg > edge_avg * 2.0 {
        println!("\n✅ SUCCESS: Network CAN learn!");
        println!("   The network successfully learned to prefer center positions.");
        println!("   This means the training code WORKS.");
        println!("\n💡 Next step: AlphaGo Zero style iterative training on real games.");
    } else {
        println!("\n❌ FAILURE: Network did NOT learn!");
        println!("   Center and edge probabilities are too similar.");
        println!("   This indicates a bug in the training code or architecture.");
        println!("\n🔍 Need to debug:");
        println!("   1. Check if gradients are flowing");
        println!("   2. Check optimizer configuration");
        println!("   3. Check loss calculation");
    }

    Ok(())
}
