//! Test si Policy Network peut apprendre
//!
//! Crée des données synthétiques BIAISÉES où la position 9 (centre)
//! apparaît 80% du temps, et teste si le policy network peut apprendre
//! cette distribution.
//!
//! Si ça marche → Problème = données self-play uniformes (circular learning)
//! Si ça marche pas → Problème = architecture ou gradients

use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use tch::{Device, Kind, Tensor};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Test: Policy Network peut-il apprendre une distribution biaisée?\n");

    // Create fresh network
    println!("📊 Creating fresh policy network...");
    let neural_config = NeuralConfig {
        input_dim: (9, 5, 5),
        nn_architecture: NNArchitecture::Cnn,
        policy_lr: 0.1, // 10x plus élevé!
        value_lr: 0.01,
        ..Default::default()
    };

    let model_path_backup = std::env::var("MODEL_PATH").ok();
    std::env::set_var("MODEL_PATH", "/nonexistent");

    let mut manager = NeuralManager::with_config(neural_config)?;

    if let Some(path) = model_path_backup {
        std::env::set_var("MODEL_PATH", path);
    }

    println!("✅ Fresh network created\n");

    // Create synthetic BIASED data
    // Position 9 (center) should appear 80% of the time
    println!("📊 Creating BIASED synthetic data:");
    println!("   Position 9 (center): 80% probability");
    println!("   Other positions: 20% / 18 = 1.1% each\n");

    let device = Device::Cpu;
    let batch_size = 100;

    // Create VARIED board states (not all zeros!)
    // Each state should be different so network can learn spatial patterns
    let mut states_vec = Vec::new();
    for i in 0..batch_size {
        // Create a random-ish state by filling some positions
        let mut state = vec![0.0f32; 8 * 5 * 5];

        // Fill some random positions with values
        for j in 0..10 {
            let pos = ((i * 7 + j * 3) % 19) as usize;
            let row = pos / 5;
            let col = pos % 5;
            let idx = row * 5 + col;

            // Set some channel values (simulate tiles)
            state[idx] = ((i + j) % 10) as f32 / 10.0; // value1
            state[25 + idx] = ((i + j * 2) % 10) as f32 / 10.0; // value2
            state[2 * 25 + idx] = ((i + j * 3) % 10) as f32 / 10.0; // value3
            state[3 * 25 + idx] = 1.0; // occupied
        }

        states_vec.extend(state);
    }

    let states = Tensor::from_slice(&states_vec)
        .view([batch_size, 8, 5, 5])
        .to_device(device);

    // Biased policy targets: 80% position 9, 20% others
    let mut policy_targets = Vec::new();
    for i in 0..batch_size {
        if i < 80 {
            policy_targets.push(9i64); // 80% center
        } else {
            policy_targets.push(i % 19); // 20% spread across others
        }
    }
    let policy_targets = Tensor::from_slice(&policy_targets).to_device(device);

    println!("🏋️ Training for 100 epochs on biased data...\n");

    for epoch in 0..100 {
        let policy_net = manager.policy_net();
        let policy_pred = policy_net.forward(&states, true);
        let policy_loss = policy_pred.cross_entropy_for_logits(&policy_targets);

        let policy_opt = manager.policy_optimizer_mut();
        policy_opt.backward_step(&policy_loss);

        if (epoch + 1) % 10 == 0 {
            let p_loss: f64 = policy_loss.double_value(&[]);
            println!("  Epoch {}: policy_loss={:.4}", epoch + 1, p_loss);
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
        let marker = if pos == 9 {
            " <- CENTER (should be HIGH ~80%)"
        } else {
            ""
        };
        println!("  Position {:2}: {:.4}{}", pos, prob, marker);
    }

    let center_prob = probs[9];
    let avg_other = probs
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 9)
        .map(|(_, &p)| p)
        .sum::<f64>()
        / 18.0;

    println!("\n🎯 Results:");
    println!("   Center (pos 9): {:.4} (target: 0.80)", center_prob);
    println!("   Others avg:     {:.4} (target: 0.011)", avg_other);
    println!("   Ratio:          {:.2}x", center_prob / avg_other);

    if center_prob > 0.4 {
        println!("\n✅ SUCCESS: Policy network CAN learn!");
        println!("   Le réseau a appris à préférer la position centrale.");
        println!("   → PROBLÈME = Données self-play uniformes (circular learning)");
        println!("\n💡 Solution:");
        println!("   1. Augmenter weight_rollout/heuristic pour guider MCTS");
        println!("   2. Utiliser temperature sampling dans MCTS");
        println!("   3. Augmenter LR policy (0.05-0.1)");
        println!("   4. Ou abandonner approche réseau");
    } else {
        println!("\n❌ FAILURE: Policy network did NOT learn!");
        println!(
            "   Position centrale: {:.4} (pas assez élevée)",
            center_prob
        );
        println!("   → PROBLÈME = Architecture ou optimizer");
        println!("\n🔍 Need to debug:");
        println!("   1. Check gradient flow");
        println!("   2. Try higher learning rate");
        println!("   3. Check policy network architecture");
    }

    Ok(())
}
