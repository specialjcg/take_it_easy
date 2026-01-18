///! Analyze what strategy produces a score of 23 pts

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::Plateau;
use take_it_easy::game::remove_tile_from_deck::replace_tile_in_deck;
use take_it_easy::game::tile::Tile;
use take_it_easy::scoring::scoring::result;

fn main() {
    println!("🔍 Analyzing score of 23 pts\n");

    // Strategy 1: Fill only positions 0-5 (top positions based on logits)
    println!("📋 Strategy 1: Fill positions 0-5 only");
    let mut plateau1 = Plateau {
        tiles: vec![Tile(0, 0, 0); 19],
    };

    // Place 6 tiles at positions 0-5
    let test_tiles = vec![
        Tile(1, 2, 3),
        Tile(5, 6, 7),
        Tile(9, 7, 8),
        Tile(1, 6, 4),
        Tile(5, 2, 8),
        Tile(9, 6, 3),
    ];

    for (i, &tile) in test_tiles.iter().enumerate() {
        plateau1.tiles[i] = tile;
    }

    let score1 = result(&plateau1);
    println!("Score with positions 0-5 filled: {}", score1);
    println!("Plateau: {:?}\n", plateau1.tiles[0..6].to_vec());

    // Strategy 2: Simulate what CNN would actually do
    println!("📋 Strategy 2: Always choose position 4 (highest logit ~1.3-1.7)");
    let mut plateau2 = Plateau {
        tiles: vec![Tile(0, 0, 0); 19],
    };
    let mut deck = create_deck();

    for turn in 0..19 {
        let tiles = deck.tiles();
        if tiles.is_empty() {
            break;
        }
        let tile_idx = turn % tiles.len();
        let tile = tiles[tile_idx];
        let new_deck = replace_tile_in_deck(&deck, &tile);
        deck = new_deck;

        // Always choose position 4 if legal, else position 0-5
        let legal_moves = get_legal_moves(&plateau2);
        let preferred = [4, 3, 2, 1, 0, 5, 6, 7, 8, 9];

        let chosen_pos = preferred.iter()
            .find(|&&pos| legal_moves.contains(&pos))
            .copied()
            .unwrap_or(legal_moves[0]);

        plateau2.tiles[chosen_pos] = tile;
    }

    let score2 = result(&plateau2);
    println!("Score with greedy bias toward positions 0-5: {}", score2);

    // Strategy 3: Check if 23 = one complete line
    println!("\n📋 Strategy 3: Common line scores");
    println!("Line with all 1s: {}", 1*7);
    println!("Line with all 5s: {}", 5*7);
    println!("Line with all 9s: {}", 9*7);
    println!("Line with 1,1,5,5,5,9,9: {}", 1+1+5+5+5+9+9);
    println!("Line with 1,5,5,5,5,5,9: {}", 1+5+5+5+5+5+9);

    // Visualize plateau layout
    println!("\n📊 Plateau position layout (5x5 grid, 19 hexagonal positions):");
    println!("     0   1");
    println!("   2   3   4");
    println!(" 5   6   7   8");
    println!("   9  10  11");
    println!("    12  13");
    println!();
    println!("    14  15");
    println!("  16  17  18");

    println!("\n🔍 Analysis:");
    println!("If positions 0-5 are heavily favored:");
    println!("  Positions 0,1,2,3,4,5 form a diagonal or cluster");
    println!("  This could complete specific lines while leaving others empty");
}
