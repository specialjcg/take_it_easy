///! Debug CNN predictions to understand why all games score 23 pts
use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::Plateau;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::manager::NeuralManager;
use take_it_easy::neural::tensor_conversion::convert_plateau_to_tensor;
use tch::Kind;

fn main() {
    println!("🔍 CNN Predictions Debug\n");

    // Load CNN
    let neural_manager = NeuralManager::new().expect("Failed to load CNN");

    let policy_net = neural_manager.policy_net();

    println!("✅ CNN loaded successfully\n");

    // Test 1: Empty plateau at turn 0
    println!("📋 Test 1: Empty plateau, tile (1,2,3)");
    let plateau = Plateau {
        tiles: vec![Tile(0, 0, 0); 19],
    };
    let tile = Tile(1, 2, 3);
    let deck = create_deck();

    let state = convert_plateau_to_tensor(&plateau, &tile, &deck, 0, 19);
    let policy = policy_net.forward(&state, false);
    let probs: Vec<f32> = policy.view([-1]).to_kind(Kind::Float).try_into().unwrap();

    println!("State shape: {:?}", state.size());
    println!("Policy output shape: {:?}", policy.size());
    println!("\nPredicted probabilities (19 positions):");
    for (pos, &prob) in probs.iter().enumerate() {
        println!("  Position {}: {:.6}", pos, prob);
    }

    let max_prob = probs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let min_prob = probs.iter().cloned().fold(f32::INFINITY, f32::min);
    let sum_prob: f32 = probs.iter().sum();
    let mean_prob = sum_prob / probs.len() as f32;

    println!(
        "\nStats: max={:.6}, min={:.6}, mean={:.6}, sum={:.6}",
        max_prob, min_prob, mean_prob, sum_prob
    );

    // Test 2: Same tile, different state
    println!("\n\n📋 Test 2: One tile placed, same tile (1,2,3)");
    let mut plateau2 = Plateau {
        tiles: vec![Tile(0, 0, 0); 19],
    };
    plateau2.tiles[5] = Tile(5, 6, 7); // Place a tile at position 5

    let state2 = convert_plateau_to_tensor(&plateau2, &tile, &deck, 1, 19);
    let policy2 = policy_net.forward(&state2, false);
    let probs2: Vec<f32> = policy2.view([-1]).to_kind(Kind::Float).try_into().unwrap();

    println!("Predicted probabilities:");
    for (pos, &prob) in probs2.iter().enumerate() {
        println!("  Position {}: {:.6}", pos, prob);
    }

    let max_prob2 = probs2.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let min_prob2 = probs2.iter().cloned().fold(f32::INFINITY, f32::min);
    let mean_prob2 = probs2.iter().sum::<f32>() / probs2.len() as f32;

    println!(
        "\nStats: max={:.6}, min={:.6}, mean={:.6}",
        max_prob2, min_prob2, mean_prob2
    );

    // Test 3: Different tile
    println!("\n\n📋 Test 3: Empty plateau, different tile (9,7,8)");
    let tile3 = Tile(9, 7, 8);
    let state3 = convert_plateau_to_tensor(&plateau, &tile3, &deck, 0, 19);
    let policy3 = policy_net.forward(&state3, false);
    let probs3: Vec<f32> = policy3.view([-1]).to_kind(Kind::Float).try_into().unwrap();

    println!("Predicted probabilities:");
    for (pos, &prob) in probs3.iter().enumerate() {
        println!("  Position {}: {:.6}", pos, prob);
    }

    let max_prob3 = probs3.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let min_prob3 = probs3.iter().cloned().fold(f32::INFINITY, f32::min);
    let mean_prob3 = probs3.iter().sum::<f32>() / probs3.len() as f32;

    println!(
        "\nStats: max={:.6}, min={:.6}, mean={:.6}",
        max_prob3, min_prob3, mean_prob3
    );

    // Check if predictions are identical
    println!("\n\n🔬 Comparing predictions:");
    let same_12 = probs.iter().zip(&probs2).all(|(a, b)| (a - b).abs() < 1e-5);
    let same_13 = probs.iter().zip(&probs3).all(|(a, b)| (a - b).abs() < 1e-5);

    println!("  Test1 == Test2: {}", same_12);
    println!("  Test1 == Test3: {}", same_13);

    if same_12 && same_13 {
        println!("\n❌ PROBLEM: Network predicts IDENTICAL distributions regardless of input!");
        println!("   This explains why all games score 23 pts.");
        println!("   Likely causes:");
        println!("   - Model weights not properly loaded");
        println!("   - Model collapsed to trivial solution");
        println!("   - Input encoding bug (all states look the same)");
    } else {
        println!("\n✅ Network responds to different inputs");
    }

    println!("\n✅ Debug complete");
}
