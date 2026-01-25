//! Test Neural Network Forward Pass
//!
//! Verifies that the neural network produces sensible outputs

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::tensor_conversion::convert_plateau_to_tensor;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use tch::IndexOp;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔬 Testing Neural Network Forward Pass\n");

    // Initialize network
    let neural_config = NeuralConfig {
        input_dim: (9, 5, 5),
        nn_architecture: NNArchitecture::Cnn,
        ..Default::default()
    };
    let manager = NeuralManager::with_config(neural_config)?;

    // Create test state
    let plateau = create_plateau_empty();
    let deck = create_deck();
    let tile = Tile(5, 7, 3); // Arbitrary tile

    // Convert to tensor
    let tensor = convert_plateau_to_tensor(&plateau, &tile, &deck, 0, 19);

    println!("📊 Input Tensor:");
    println!("   Shape: {:?}", tensor.size());
    println!("   Min: {:.4}", tensor.min().double_value(&[]));
    println!("   Max: {:.4}", tensor.max().double_value(&[]));
    println!(
        "   Mean: {:.4}",
        tensor.mean(tch::Kind::Float).double_value(&[])
    );

    // Test Policy Network
    let policy_net = manager.policy_net();
    let policy_logits = policy_net.forward(&tensor, false);
    let policy = policy_logits.log_softmax(-1, tch::Kind::Float).exp();

    println!("\n📊 Policy Network Output:");
    println!("   Shape: {:?}", policy.size());
    println!(
        "   Sum: {:.4} (should be ~1.0)",
        policy.sum(tch::Kind::Float).double_value(&[])
    );
    println!("   Min: {:.6}", policy.min().double_value(&[]));
    println!("   Max: {:.6}", policy.max().double_value(&[]));
    println!(
        "   Mean: {:.6}",
        policy.mean(tch::Kind::Float).double_value(&[])
    );

    // Print top 5 actions
    let policy_values: Vec<f64> = (0..19)
        .map(|i| policy.i((0, i as i64)).double_value(&[]))
        .collect();

    let mut indexed: Vec<(usize, f64)> = policy_values
        .iter()
        .enumerate()
        .map(|(i, &v)| (i, v))
        .collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    println!("\n   Top 5 actions:");
    for (pos, prob) in indexed.iter().take(5) {
        println!("      Position {}: {:.6}", pos, prob);
    }

    // Test Value Network
    let value_net = manager.value_net();
    let value = value_net.forward(&tensor, false);

    println!("\n📊 Value Network Output:");
    println!("   Shape: {:?}", value.size());
    println!(
        "   Value: {:.6} (should be in [-1, 1])",
        value.double_value(&[])
    );

    // Test on a partially filled plateau
    println!("\n\n🔬 Testing on Partially Filled Plateau\n");

    let mut plateau2 = create_plateau_empty();
    plateau2.tiles[0] = Tile(5, 7, 3);
    plateau2.tiles[4] = Tile(9, 8, 6);
    plateau2.tiles[9] = Tile(7, 5, 4);

    let tensor2 = convert_plateau_to_tensor(&plateau2, &tile, &deck, 5, 19);

    let policy2 = policy_net
        .forward(&tensor2, false)
        .log_softmax(-1, tch::Kind::Float)
        .exp();
    let value2 = value_net.forward(&tensor2, false);

    println!("📊 Policy (partial plateau):");
    println!(
        "   Sum: {:.4}",
        policy2.sum(tch::Kind::Float).double_value(&[])
    );

    println!("\n📊 Value (partial plateau):");
    println!("   Value: {:.6}", value2.double_value(&[]));

    println!("\n\n🎯 Diagnosis:");

    let policy_sum = policy.sum(tch::Kind::Float).double_value(&[]);
    let policy_entropy = -(policy.shallow_clone() * policy.log())
        .sum(tch::Kind::Float)
        .double_value(&[]);
    let value_val = value.double_value(&[]);

    if (policy_sum - 1.0).abs() > 0.01 {
        println!("   ❌ Policy sum = {:.4} (should be 1.0)", policy_sum);
    } else {
        println!("   ✅ Policy sums to 1.0");
    }

    if policy_entropy < 0.1 {
        println!(
            "   ⚠️  Policy entropy = {:.4} (very low - almost deterministic)",
            policy_entropy
        );
    } else {
        println!("   ✅ Policy entropy = {:.4} (reasonable)", policy_entropy);
    }

    if value_val.abs() > 1.0 {
        println!("   ❌ Value = {:.4} (outside [-1, 1] range)", value_val);
    } else {
        println!("   ✅ Value in [-1, 1] range");
    }

    // Check if policy is uniform
    let expected_uniform = 1.0 / 19.0;
    let max_prob = indexed[0].1;

    if (max_prob - expected_uniform).abs() < 0.01 {
        println!("   ⚠️  Policy is nearly UNIFORM (network not trained?)");
        println!(
            "      Max prob = {:.4}, Expected uniform = {:.4}",
            max_prob, expected_uniform
        );
    } else {
        println!("   ✅ Policy is non-uniform (shows preference)");
    }

    Ok(())
}
