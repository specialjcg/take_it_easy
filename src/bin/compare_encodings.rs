/// Compare old normalized encoding vs new one-hot encoding
///
/// This tool visualizes the difference to understand why one-hot is better
use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::tensor_conversion::convert_plateau_to_tensor;
use take_it_easy::neural::tensor_onehot::{convert_plateau_onehot, ONEHOT_CHANNELS};
use tch::IndexOp;

fn main() {
    println!("=== ENCODING COMPARISON: Normalized vs One-Hot ===\n");

    // Create a test scenario
    let mut plateau = create_plateau_empty();
    let deck = create_deck();

    // Place some tiles to create a pattern
    // Line Dir1 (vertical column 0): positions 0, 1, 2 - all need same Dir1 value
    plateau.tiles[0] = Tile(5, 6, 3); // Dir1=5
    plateau.tiles[1] = Tile(5, 2, 4); // Dir1=5 (matching!)
                                      // Position 2 empty - should place Dir1=5 to complete

    // Current tile to evaluate
    let good_tile = Tile(5, 7, 8); // Dir1=5 - MATCHES line!
    let bad_tile = Tile(9, 6, 3); // Dir1=9 - CONFLICTS with line!

    println!("Scenario: Partial line with Dir1=5 at positions 0,1");
    println!("Position 2 is empty - can complete line\n");

    // Show the placement
    println!("Plateau state:");
    println!("  Position 0: Tile(5,6,3) - Dir1=5");
    println!("  Position 1: Tile(5,2,4) - Dir1=5");
    println!("  Position 2: EMPTY\n");

    // Compare encodings for GOOD tile (Dir1=5)
    println!("=== GOOD TILE: (5, 7, 8) - Dir1 matches line ===\n");

    let old_tensor_good = convert_plateau_to_tensor(&plateau, &good_tile, &deck, 2, 19);
    let new_tensor_good = convert_plateau_onehot(&plateau, &good_tile, &deck, 2);

    println!("Old Encoding (47 channels, normalized values):");
    println!("  Ch 0 (Dir1 values): 5/9 = {:.3}", 5.0 / 9.0);
    println!("  Ch 4 (Current tile Dir1): 5/9 = {:.3}", 5.0 / 9.0);
    println!("  Problem: Network must learn that 0.556 == 0.556 means 'match'\n");

    println!("New One-Hot Encoding (37 channels):");
    println!("  Ch 0-2 (Dir1 one-hot): [1=0, 5=1, 9=0] for placed tiles");
    println!("  Ch 10-12 (Current Dir1 one-hot): [1=0, 5=1, 9=0]");
    println!("  Advantage: Dot product [0,1,0]·[0,1,0] = 1.0 = perfect match!\n");

    // Compare encodings for BAD tile (Dir1=9)
    println!("=== BAD TILE: (9, 6, 3) - Dir1 conflicts with line ===\n");

    let _old_tensor_bad = convert_plateau_to_tensor(&plateau, &bad_tile, &deck, 2, 19);
    let _new_tensor_bad = convert_plateau_onehot(&plateau, &bad_tile, &deck, 2);

    println!("Old Encoding (normalized values):");
    println!("  Ch 0 (Dir1 placed): 5/9 = {:.3}", 5.0 / 9.0);
    println!("  Ch 4 (Current tile Dir1): 9/9 = {:.3}", 9.0 / 9.0);
    println!(
        "  Problem: 0.556 vs 1.0 - difference is {:.3}",
        1.0 - 5.0 / 9.0
    );
    println!("           Network must learn this difference means 'conflict'\n");

    println!("New One-Hot Encoding:");
    println!("  Ch 0-2 (Dir1 placed): [1=0, 5=1, 9=0]");
    println!("  Ch 10-12 (Current Dir1): [1=0, 5=0, 9=1]");
    println!("  Advantage: Dot product [0,1,0]·[0,0,1] = 0.0 = NO match!\n");

    // Show actual tensor values at position 2 (where we'd place)
    println!("=== TENSOR VALUES AT POSITION 2 (empty cell) ===\n");

    // Position 2 in hex maps to grid (3,0) = row 3, col 0 = index 15
    let grid_idx = 3 * 5; // = 15

    println!("Grid index for hex pos 2: {}", grid_idx);
    println!("\nOld encoding at position 2:");
    print_channel_values(
        &old_tensor_good,
        grid_idx,
        &[(0, "Dir1 value"), (3, "Empty mask"), (4, "Current Dir1")],
    );

    println!("\nNew one-hot encoding at position 2:");
    print_channel_values(
        &new_tensor_good,
        grid_idx,
        &[
            (0, "Dir1=1"),
            (1, "Dir1=5"),
            (2, "Dir1=9"),
            (9, "Occupied"),
            (10, "Current Dir1=1"),
            (11, "Current Dir1=5"),
            (12, "Current Dir1=9"),
        ],
    );

    println!("\n=== WHY ONE-HOT IS BETTER ===\n");
    println!("1. PATTERN MATCHING:");
    println!("   Old: Must learn that 0.556 and 0.556 are 'same value'");
    println!("   New: Channel 1 (Dir1=5) is 1.0 everywhere → obvious pattern\n");

    println!("2. COMPATIBILITY CHECK:");
    println!("   Old: Must learn distance metric between normalized values");
    println!("   New: Simple element-wise comparison of one-hot vectors\n");

    println!("3. LINE COMPLETION:");
    println!("   Old: Convolution sees [0.556, 0.556, 0.0] - hard to detect pattern");
    println!("   New: Channel 1 has [1.0, 1.0, 0.0] - obvious '2 of 3 same value'\n");

    println!("4. LEARNING GRADIENTS:");
    println!("   Old: Gradients depend on continuous value differences");
    println!("   New: Gradients flow through discrete category membership\n");

    // Test compilation
    println!("=== TENSOR SHAPES ===");
    println!("Old tensor shape: {:?}", old_tensor_good.size());
    println!("New tensor shape: {:?}", new_tensor_good.size());
    println!("Old channels: 47");
    println!("New channels: {} (more efficient)", ONEHOT_CHANNELS);
}

fn print_channel_values(tensor: &tch::Tensor, grid_idx: usize, channels: &[(usize, &str)]) {
    for (ch, name) in channels {
        let val = tensor
            .i((0, *ch as i64, (grid_idx / 5) as i64, (grid_idx % 5) as i64))
            .double_value(&[]);
        println!("  Ch {:2} ({}): {:.3}", ch, name, val);
    }
}
