use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::Plateau;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::policy_value_net::{PolicyNet, ValueNet};
use take_it_easy::neural::tensor_conversion::convert_plateau_to_tensor;
use tch::{nn, Device};

fn main() {
    println!("=== StochZero Forward Pass Test ===\n");

    // Create networks with 17 channels
    let vs_policy = nn::VarStore::new(Device::Cpu);
    let vs_value = nn::VarStore::new(Device::Cpu);

    let policy_net = PolicyNet::new(&vs_policy, (17, 5, 5), NNArchitecture::Cnn);
    let value_net = ValueNet::new(&vs_value, (17, 5, 5), NNArchitecture::Cnn);

    println!("✓ Networks created with 17-channel input");

    // Create test data
    let plateau = Plateau {
        tiles: vec![Tile(0, 0, 0); 19],
    };
    let deck = create_deck();
    let tile = Tile(1, 2, 3);

    // Convert to tensor (should be [1, 17, 5, 5])
    let input = convert_plateau_to_tensor(&plateau, &tile, &deck, 0, 19);
    println!("✓ Tensor shape: {:?}", input.size());

    // Verify it's 17 channels
    assert_eq!(
        input.size()[1],
        17,
        "Expected 17 channels, got {}",
        input.size()[1]
    );

    // Test forward passes
    let policy_output = policy_net.forward(&input, false);
    println!("✓ Policy output shape: {:?}", policy_output.size());
    assert_eq!(policy_output.size()[1], 19, "Expected 19 actions");

    let value_output = value_net.forward(&input, false);
    println!("✓ Value output shape: {:?}", value_output.size());
    assert_eq!(value_output.size()[1], 1, "Expected scalar value");

    println!("\n=== Test PASSED ===");
    println!("StochZero encoding works correctly!");
}
