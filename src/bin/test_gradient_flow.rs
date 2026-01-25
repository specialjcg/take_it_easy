//! Test diagnostique : Vérifie si les poids du policy network sont mis à jour

use flexi_logger::Logger;
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use tch::{Device, Tensor};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    log::info!("🔍 Test Gradient Flow - Policy Network");

    // Initialize network
    let neural_config = NeuralConfig {
        input_dim: (9, 5, 5),
        nn_architecture: NNArchitecture::Cnn,
        policy_lr: 0.01,
        value_lr: 0.001,
        ..Default::default()
    };
    let mut manager = NeuralManager::with_config(neural_config)?;

    // Get initial weights snapshot
    log::info!("\n📸 Snapshot initial des poids policy");
    let vs = manager.policy_varstore();
    let initial_weights: Vec<(String, Tensor)> = vs
        .variables()
        .iter()
        .map(|(name, tensor)| (name.clone(), tensor.shallow_clone()))
        .collect();

    log::info!("   Nombre de tensors: {}", initial_weights.len());
    for (name, tensor) in &initial_weights {
        let size: Vec<i64> = tensor.size();
        let mean = tensor.mean(tch::Kind::Float).double_value(&[]);
        let std = tensor.std(false).double_value(&[]);
        log::info!(
            "   {} - shape {:?}, mean={:.6}, std={:.6}",
            name,
            size,
            mean,
            std
        );
    }

    // Create simple training batch
    let device = Device::Cpu;
    let batch_size = 32;

    // Random states (9 channels × 5 × 5)
    let states = Tensor::randn([batch_size, 9, 5, 5], (tch::Kind::Float, device));

    // Targets: first 16 examples → position 0, next 16 → position 10
    let mut targets_vec = vec![0i64; 16];
    targets_vec.extend(vec![10i64; 16]);
    let targets = Tensor::from_slice(&targets_vec).to_device(device);

    log::info!("\n🏋️ Training for 10 epochs");

    for epoch in 0..10 {
        // Forward pass
        let policy_net = manager.policy_net();
        let policy_pred = policy_net.forward(&states, true);

        // Compute loss
        let loss = policy_pred.cross_entropy_for_logits(&targets);
        let loss_value = f64::try_from(&loss)?;

        // Backward pass
        let policy_opt = manager.policy_optimizer_mut();
        policy_opt.backward_step(&loss);

        log::info!("   Epoch {}: loss={:.6}", epoch + 1, loss_value);

        // Check predictions (skip for now to simplify)
        if epoch == 0 || epoch == 9 {
            log::info!("      (Prediction check - see loss decrease)");
        }
    }

    // Get final weights and compare
    log::info!("\n📊 Comparaison poids initial vs final");
    let vs = manager.policy_varstore();
    let final_weights = vs.variables();

    let mut weights_changed = false;
    for (idx, (name, final_tensor)) in final_weights.iter().enumerate() {
        let initial_tensor = &initial_weights[idx].1;

        // Compute difference
        let diff = (final_tensor - initial_tensor).abs().max();
        let diff_value = diff.double_value(&[]);

        let changed = diff_value > 1e-7;
        if changed {
            weights_changed = true;
        }

        let status = if changed {
            "✅ CHANGED"
        } else {
            "❌ UNCHANGED"
        };
        log::info!("   {} - max_diff={:.2e} {}", name, diff_value, status);
    }

    log::info!("\n🎯 Résultat:");
    if weights_changed {
        log::info!("   ✅ Les poids ont changé - gradients OK");
    } else {
        log::info!("   ❌ Les poids n'ont PAS changé - BUG GRADIENT!");
    }

    Ok(())
}
