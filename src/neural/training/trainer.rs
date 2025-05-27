use tch::{nn, IndexOp, Tensor};
use tch::nn::Optimizer;
use crate::mcts::mcts_result::MCTSResult;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::neural::training::gradient_clipping::enhanced_gradient_clipping;
use crate::neural::training::normalization::robust_state_normalization;

pub fn train_network_with_game_data(
    vs_policy: &nn::VarStore,
    vs_value: &nn::VarStore,
    game_data: &[MCTSResult],
    discount_factor: f64,
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
    let mut total_policy_loss = Tensor::zeros(&[], tch::kind::FLOAT_CPU);
    let mut total_value_loss = Tensor::zeros(&[], tch::kind::FLOAT_CPU);
    let mut total_entropy_loss = Tensor::zeros(&[], tch::kind::FLOAT_CPU);

    // Initialize trajectory rewards and discounted sum
    let mut trajectory_rewards = Vec::new();
    let mut discounted_sum = Tensor::zeros(&[], (tch::Kind::Float, tch::Device::Cpu));

    // === Training Loop ===
    for (step, result) in game_data.iter().rev().enumerate() {
        // 🛑 No Normalization: Use raw tensor
        let state = result.board_tensor.shallow_clone();
        let normalized_state = robust_state_normalization(&state);

        // Forward pass through networks with normalized state
        let pred_policy = policy_net.forward(&normalized_state, true).clamp_min(1e-7);
        let pred_value = value_net.forward(&normalized_state, true);

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
        let best_position = result.best_position as i64;
        let target_policy = Tensor::zeros(&[1, pred_policy.size()[1]], tch::kind::FLOAT_CPU);
        target_policy.i((0, best_position)).fill_(1.0);
        let log_policy = pred_policy.log();
        let policy_loss = -(target_policy * log_policy.shallow_clone()).sum(tch::Kind::Float);
        total_policy_loss += policy_loss;

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
    log::info!(
        "💡 Total Loss before backward: {:.4}",
        total_loss.double_value(&[])
    );

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

    log::info!(
        "🎯 Update Complete | Policy Loss: {:.4}, Value Loss: {:.4}, Entropy Loss: {:.4}",
        total_policy_loss.double_value(&[]),
        total_value_loss.double_value(&[]),
        total_entropy_loss.double_value(&[])
    );
}