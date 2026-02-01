//! Test if trained network makes sensible predictions

use flexi_logger::Logger;
use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::tensor_conversion::convert_plateau_to_tensor;
use take_it_easy::neural::{NeuralConfig, NeuralManager};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    log::info!("🧪 Testing Network Predictions");

    // Initialize network
    let neural_config = NeuralConfig {
        input_dim: (9, 5, 5),
        nn_architecture: NNArchitecture::Cnn,
        ..Default::default()
    };
    let manager = NeuralManager::with_config(neural_config)?;

    // Create test scenario: empty plateau, specific tile
    let plateau = create_plateau_empty();
    let tile = Tile(5, 7, 9); // Example tile
    let deck = create_deck();

    log::info!("\n📊 Test: Empty plateau, tile (5,7,9)");

    // Convert to tensor
    let state_tensor = convert_plateau_to_tensor(&plateau, &tile, &deck, 0, 19);
    log::info!("   State tensor shape: {:?}", state_tensor.size());

    // Get policy prediction
    let policy_pred = manager.policy_net().forward(&state_tensor, false);
    log::info!("   Policy output shape: {:?}", policy_pred.size());

    // Check if predictions are reasonable
    let policy_probs = policy_pred.softmax(-1, tch::Kind::Float);
    let policy_data: Vec<f32> = Vec::try_from(policy_probs.squeeze_dim(0))?;

    log::info!("\n📈 Policy probabilities for each position:");
    for (pos, prob) in policy_data.iter().enumerate() {
        if *prob > 0.01 {
            // Only show significant probabilities
            log::info!("   Position {}: {:.4}", pos, prob);
        }
    }

    let (max_prob, max_pos) = policy_data
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .unwrap();
    log::info!("\n   Best position: {} (prob={:.4})", max_pos, max_prob);

    // Get value prediction
    let value_pred = manager.value_net().forward(&state_tensor, false);
    let value: f32 = value_pred.double_value(&[0]) as f32;
    log::info!("   Predicted value: {:.4}", value);

    // Test another scenario with some tiles placed
    log::info!("\n📊 Test: Plateau with tiles, tile (3,6,8)");
    let mut plateau2 = create_plateau_empty();
    plateau2.tiles[0] = Tile(1, 2, 3);
    plateau2.tiles[4] = Tile(4, 5, 6);
    plateau2.tiles[9] = Tile(7, 8, 9);

    let tile2 = Tile(3, 6, 8);
    let state_tensor2 = convert_plateau_to_tensor(&plateau2, &tile2, &deck, 3, 19);

    let policy_pred2 = manager.policy_net().forward(&state_tensor2, false);
    let policy_probs2 = policy_pred2.softmax(-1, tch::Kind::Float);
    let policy_data2: Vec<f32> = Vec::try_from(policy_probs2.squeeze_dim(0))?;

    let (max_prob2, max_pos2) = policy_data2
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .unwrap();
    log::info!("   Best position: {} (prob={:.4})", max_pos2, max_prob2);

    let value_pred2 = manager.value_net().forward(&state_tensor2, false);
    let value2: f32 = value_pred2.double_value(&[0]) as f32;
    log::info!("   Predicted value: {:.4}", value2);

    log::info!("\n✅ If predictions vary and aren't uniform → network works");
    log::info!("❌ If predictions are uniform (all ~0.053) → network not learning");

    Ok(())
}
