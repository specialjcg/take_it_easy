///! Play ONE game with CNN and track all decisions
use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::Plateau;
use take_it_easy::game::remove_tile_from_deck::replace_tile_in_deck;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::manager::NeuralManager;
use take_it_easy::neural::tensor_conversion::convert_plateau_to_tensor;
use take_it_easy::scoring::scoring::result;
use tch::Kind;

fn main() {
    println!("🎮 Playing ONE game with CNN to debug score=23\n");

    let neural_manager = NeuralManager::new().expect("Failed to load CNN");
    let policy_net = neural_manager.policy_net();

    let mut plateau = Plateau {
        tiles: vec![Tile(0, 0, 0); 19],
    };
    let mut deck = create_deck();

    println!("Turn | Tile     | Legal Moves | Top 3 Logits (pos:val) | Chosen");
    println!("-----|----------|-------------|------------------------|-------");

    for turn in 0..19 {
        let tiles = deck.tiles();
        if tiles.is_empty() {
            break;
        }
        let tile_idx = turn % tiles.len();
        let tile = tiles[tile_idx];
        let new_deck = replace_tile_in_deck(&deck, &tile);
        deck = new_deck;

        let legal_moves = get_legal_moves(&plateau);
        if legal_moves.is_empty() {
            break;
        }

        // Get CNN prediction
        let state = convert_plateau_to_tensor(&plateau, &tile, &deck, turn, 19);
        let policy = policy_net.forward(&state, false);
        let policy_logits: Vec<f32> = policy
            .view([-1])
            .to_kind(Kind::Float)
            .try_into()
            .unwrap_or_else(|_| vec![]);

        // Find best legal move
        let mut best_pos = legal_moves[0];
        let mut best_logit = policy_logits
            .get(best_pos)
            .copied()
            .unwrap_or(f32::NEG_INFINITY);

        for &pos in &legal_moves {
            let logit = policy_logits.get(pos).copied().unwrap_or(f32::NEG_INFINITY);
            if logit > best_logit {
                best_logit = logit;
                best_pos = pos;
            }
        }

        // Get top 3 logits
        let mut logits_with_pos: Vec<(usize, f32)> = policy_logits
            .iter()
            .enumerate()
            .map(|(i, &v)| (i, v))
            .collect();
        logits_with_pos.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let top3 = format!(
            "{}:{:.2}, {}:{:.2}, {}:{:.2}",
            logits_with_pos[0].0,
            logits_with_pos[0].1,
            logits_with_pos[1].0,
            logits_with_pos[1].1,
            logits_with_pos[2].0,
            logits_with_pos[2].1
        );

        let legal_str = format!("{:?}", legal_moves);

        println!(
            "{:4} | ({},{},{}) | {:<11} | {} | {}",
            turn, tile.0, tile.1, tile.2, legal_str, top3, best_pos
        );

        // Place tile
        plateau.tiles[best_pos] = tile;
    }

    let score = result(&plateau);

    println!("\n📊 Final Results:");
    println!("Score: {}", score);
    println!("\nFinal plateau:");
    for (i, tile) in plateau.tiles.iter().enumerate() {
        if tile.0 != 0 {
            println!("  Position {:2}: ({},{},{})", i, tile.0, tile.1, tile.2);
        }
    }

    println!("\n🔍 Position usage pattern:");
    let positions_used: Vec<usize> = plateau
        .tiles
        .iter()
        .enumerate()
        .filter(|(_, tile)| tile.0 != 0)
        .map(|(i, _)| i)
        .collect();
    println!("Positions used: {:?}", positions_used);

    // Check which positions are 0-5
    let count_0_5 = positions_used.iter().filter(|&&p| p <= 5).count();
    println!("Positions 0-5 used: {}/6", count_0_5);
    println!(
        "Other positions used: {}/13",
        positions_used.len() - count_0_5
    );
}
