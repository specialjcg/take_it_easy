///! Test pure CNN policy (no MCTS, no rollouts) to evaluate bag awareness impact
///! This bypasses MCTS weighting to directly test if StochZero learns useful patterns

use rand::prelude::*;
use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::Plateau;
use take_it_easy::game::remove_tile_from_deck::{replace_tile_in_deck, get_available_tiles};
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::manager::NeuralManager;
use take_it_easy::neural::tensor_conversion::convert_plateau_to_tensor;
use take_it_easy::scoring::scoring::result;
use tch::Kind;

fn main() {
    println!("🎯 Pure CNN Policy Test (No MCTS/Rollouts)");
    println!("Tests if StochZero bag awareness improves greedy policy\n");

    // Load StochZero CNN
    let neural_manager = NeuralManager::new()
        .expect("Failed to load CNN");

    let policy_net = neural_manager.policy_net();

    let num_games = 30;
    let mut scores = Vec::new();
    let mut rng = rand::rng();

    println!("Playing {} games with pure CNN policy...\n", num_games);

    for game_idx in 0..num_games {
        let mut plateau = Plateau {
            tiles: vec![Tile(0, 0, 0); 19],
        };
        let mut deck = create_deck();

        // Shuffle the deck for each game to get varied play
        deck.tiles_mut().shuffle(&mut rng);

        // Debug first game
        let debug = game_idx == 0;
        if debug {
            println!("\n🔍 DEBUG Game 1:");
        }

        for turn in 0..19 {
            // Draw random tile from available (non-empty) tiles
            let available = get_available_tiles(&deck);
            if available.is_empty() {
                break;
            }
            let tile_idx = rng.random_range(0..available.len());
            let tile = available[tile_idx];
            let new_deck = replace_tile_in_deck(&deck, &tile);
            deck = new_deck;

            // Get legal moves
            let legal_moves = get_legal_moves(&plateau);
            if legal_moves.is_empty() {
                break;
            }

            // Get CNN policy prediction
            let state = convert_plateau_to_tensor(&plateau, &tile, &deck, turn, 19);
            let policy = policy_net.forward(&state, false);
            let policy_probs: Vec<f32> = policy.view([-1])
                .to_kind(Kind::Float)
                .try_into()
                .unwrap_or_else(|_| vec![]);

            // Choose move with highest probability (greedy)
            let mut best_pos = legal_moves[0];
            let mut best_prob = policy_probs.get(best_pos).copied().unwrap_or(0.0);

            for &pos in &legal_moves {
                let prob = policy_probs.get(pos).copied().unwrap_or(0.0);
                if prob > best_prob {
                    best_prob = prob;
                    best_pos = pos;
                }
            }

            if debug && turn < 5 {
                println!("  Turn {}: tile={:?}, legal={:?}, chose pos {} (logit={:.2})",
                         turn, tile, &legal_moves[..legal_moves.len().min(5)], best_pos, best_prob);
            }

            // Place tile
            plateau.tiles[best_pos] = tile;
        }

        if debug {
            println!("  Final board:");
            for i in 0..19 {
                if plateau.tiles[i] != Tile(0, 0, 0) {
                    print!("    pos {}: {:?}", i, plateau.tiles[i]);
                    if (i + 1) % 5 == 0 { println!(); }
                }
            }
            println!();
        }

        let score = result(&plateau);
        scores.push(score);

        if (game_idx + 1) % 5 == 0 {
            let avg: f32 = scores.iter().sum::<i32>() as f32 / scores.len() as f32;
            println!("  Game {}/{}: score={} (avg so far: {:.1})",
                     game_idx + 1, num_games, score, avg);
        }
    }

    // Statistics
    let mean: f32 = scores.iter().sum::<i32>() as f32 / scores.len() as f32;
    let min = *scores.iter().min().unwrap();
    let max = *scores.iter().max().unwrap();

    let variance: f32 = scores.iter()
        .map(|&s| {
            let diff = s as f32 - mean;
            diff * diff
        })
        .sum::<f32>() / scores.len() as f32;
    let std_dev = variance.sqrt();

    println!("\n📊 Pure CNN Policy Results:");
    println!("   Average score: {:.2} ± {:.2}", mean, std_dev);
    println!("   Range: [{}, {}]", min, max);
    println!("   Games played: {}", scores.len());

    println!("\n🔍 Comparison:");
    println!("   Pure MCTS rollouts: 84-88 pts");
    println!("   Baseline CNN (8 ch): 147-152 pts");
    println!("   StochZero pure policy: {:.1} pts", mean);

    if mean < 100.0 {
        println!("\n⚠️  Pure policy is weak! Network needs more training.");
    } else if mean < 140.0 {
        println!("\n⚠️  Bag awareness not helping much. Check encoding or training.");
    } else {
        println!("\n✅ Network learned useful patterns!");
    }
}
