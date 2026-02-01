use crate::mcts::mcts_result::MCTSResult;
use crate::neural::gnn::convert_plateau_for_gnn;
use crate::neural::manager::NNArchitecture;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::neural::training::gradient_clipping::enhanced_gradient_clipping;
use crate::neural::training::normalization::robust_state_normalization;
use tch::nn::Optimizer;
use tch::{nn, Tensor};

#[allow(clippy::too_many_arguments)]
pub fn train_network_with_game_data(
    vs_policy: &nn::VarStore,
    vs_value: &nn::VarStore,
    game_data: &[MCTSResult],
    _discount_factor: f64,
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    optimizer_policy: &mut Optimizer,
    optimizer_value: &mut Optimizer,
) {
    // Hyperparameters
    let entropy_weight = 0.05;
    let gamma = 0.99;
    let epsilon = 1e-8;

    // Initialize accumulators
    let mut predictions = Vec::new();
    let mut targets = Vec::new();
    let mut total_policy_loss = Tensor::zeros([], tch::kind::FLOAT_CPU);
    let mut total_value_loss = Tensor::zeros([], tch::kind::FLOAT_CPU);
    let mut total_entropy_loss = Tensor::zeros([], tch::kind::FLOAT_CPU);

    // Initialize trajectory rewards and discounted sum
    let mut trajectory_rewards = Vec::new();
    let mut discounted_sum = Tensor::zeros([], (tch::Kind::Float, tch::Device::Cpu));

    // === Training Loop ===
    let PolicyNet { arch, .. } = &policy_net;
    let arch = *arch;
    for (step, result) in game_data.iter().rev().enumerate() {
        // 🛑 No Normalization: Use raw tensor
        let state = result.board_tensor.shallow_clone();
        let normalized_state = robust_state_normalization(&state);
        let (input_policy, input_value) = match arch {
            NNArchitecture::Cnn | NNArchitecture::CnnOnehot => {
                (normalized_state.shallow_clone(), normalized_state)
            }
            NNArchitecture::Gnn => {
                let plateau_ref = result.plateau.as_ref().expect("MCTSResult.plateau is None");
                let current_turn = result
                    .current_turn
                    .expect("MCTSResult.current_turn is None");
                let total_turns = result.total_turns.expect("MCTSResult.total_turns is None");
                let gnn_feat = convert_plateau_for_gnn(plateau_ref, current_turn, total_turns);
                (gnn_feat.shallow_clone(), gnn_feat)
            }
        };
        // Forward pass through networks with normalized state
        let pred_policy = policy_net.forward(&input_policy, true).clamp_min(1e-7);
        let pred_value = value_net.forward(&input_value, true);

        // Forward pass through networks with normalized state
        // Normalize reward: divide by a constant max value (e.g., 100)
        let reward = Tensor::from(result.subscore).to_kind(tch::Kind::Float) / 100.0;
        let gamma_tensor = Tensor::from_slice(&[gamma]).to_kind(tch::Kind::Float);

        // ✅ NaN & Inf Check for reward
        if reward.isnan().any().double_value(&[]) > 0.0
            || reward.isinf().any().double_value(&[]) > 0.0
        {
            log::error!("⚠️ NaN or Inf detected in reward at step {}", step);
            continue;
        }

        // Update discounted sum with normalized reward
        discounted_sum = reward + gamma_tensor * discounted_sum;

        // ✅ NaN & Inf Check for discounted sum
        if discounted_sum.isnan().any().double_value(&[]) > 0.0
            || discounted_sum.isinf().any().double_value(&[]) > 0.0
        {
            log::error!("⚠️ NaN or Inf detected in discounted sum at step {}", step);
            continue;
        }

        // Store the value for analysis
        trajectory_rewards.push(discounted_sum.double_value(&[]));

        // Generate target tensor directly from discounted sum
        let discounted_reward = discounted_sum.shallow_clone();

        // Append for later analysis
        predictions.push(pred_value.double_value(&[]));
        targets.push(discounted_reward.double_value(&[]));

        // === Compute Losses ===
        // Policy loss
        let policy_len = pred_policy.size()[1] as usize;
        let mut policy_vec = vec![0f32; policy_len];
        let flattened_policy = result
            .policy_distribution
            .to_kind(tch::Kind::Float)
            .flatten(0, -1);
        let numel = flattened_policy.numel().max(0);
        if numel > 0 {
            let mut buffer = vec![0f32; numel];
            flattened_policy.copy_data(&mut buffer, numel);
            for (idx, value) in buffer.into_iter().take(policy_vec.len()).enumerate() {
                policy_vec[idx] = value;
            }
        }

        let sum: f32 = policy_vec.iter().sum();
        if sum <= f32::EPSILON {
            if !policy_vec.is_empty() {
                let best_position = result.best_position.min(policy_vec.len() - 1);
                policy_vec[best_position] = 1.0;
            }
        } else {
            for value in &mut policy_vec {
                *value /= sum;
            }
        }

        let target_policy = Tensor::from_slice(&policy_vec).view([1, policy_len as i64]);
        let log_policy = pred_policy.log();
        let policy_loss = -(target_policy * log_policy.shallow_clone()).sum(tch::Kind::Float);
        total_policy_loss += policy_loss;

        if log::log_enabled!(log::Level::Trace) {
            let boosted = result
                .policy_distribution_boosted
                .to_kind(tch::Kind::Float)
                .flatten(0, -1);
            let boosted_len = boosted.numel().max(0);
            if boosted_len == policy_vec.len() {
                let mut boosted_vec = vec![0f32; boosted_len];
                boosted.copy_data(&mut boosted_vec, boosted_len);
                let mut kl = 0.0f32;
                for (p, q) in policy_vec.iter().zip(boosted_vec.iter()) {
                    let p = (p + 1e-6).clamp(1e-6, 1.0);
                    let q = (q + 1e-6).clamp(1e-6, 1.0);
                    kl += p * (p / q).ln();
                }
                log::trace!("[Policy KL] step={} kl={:.6}", step, kl);
            }
        }

        // Entropy loss
        let entropy_loss = -(pred_policy * (log_policy + epsilon)).sum(tch::Kind::Float);
        total_entropy_loss += entropy_loss;

        // Value loss (Huber loss for better stability)
        let diff = discounted_reward.shallow_clone() - pred_value.shallow_clone();
        let abs_diff = diff.abs();
        let delta = 1.0;
        let value_loss = abs_diff.le(delta).to_kind(tch::Kind::Float) * 0.5 * &diff * &diff
            + abs_diff.gt(delta).to_kind(tch::Kind::Float) * (delta * (&abs_diff - 0.5 * delta));
        total_value_loss += value_loss.mean(tch::Kind::Float);
    }

    // Fix: Add explicit type annotation for total_loss
    let total_loss: Tensor = total_policy_loss.shallow_clone()
        + total_value_loss.shallow_clone()
        + (entropy_weight * total_entropy_loss.shallow_clone());

    // Log the loss before backpropagation
    // ✅ Enhanced NaN and Inf check before backpropagation
    if total_loss.isnan().any().double_value(&[]) > 0.0 {
        log::error!("⚠️ NaN detected in total loss! Skipping backpropagation.");
        return;
    }
    if total_loss.isinf().any().double_value(&[]) > 0.0 {
        log::error!("⚠️ Inf detected in total loss! Skipping backpropagation.");
        return;
    }

    // Check if total_loss requires gradients before calling backward
    if !total_loss.requires_grad() {
        log::error!("⚠️ Total loss does not require gradients! Skipping backpropagation.");
        return;
    }

    total_loss.backward();

    // Utilisez :
    let gradient_result = enhanced_gradient_clipping(vs_value, vs_policy);
    let _max_grad_value = gradient_result.max_grad_value;
    let _max_grad_policy = gradient_result.max_grad_policy;

    // === Optimizer Step ===
    optimizer_policy.step();
    optimizer_policy.zero_grad();
    optimizer_value.step();
    optimizer_value.zero_grad();
}
