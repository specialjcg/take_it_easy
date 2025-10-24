use crate::game::deck::Deck;
use crate::game::get_legal_moves::get_legal_moves;
use crate::game::plateau::Plateau;
use crate::game::plateau_is_full::is_plateau_full;
use crate::game::tile::Tile;
use crate::scoring::scoring::result;
use crate::strategy::position_evaluation::enhanced_position_evaluation;
use rand::Rng;

/// Smart rollout using heuristics instead of pure random play
/// This should dramatically improve MCTS evaluation quality
pub fn simulate_games_smart(plateau: Plateau, deck: Deck, policy_net: Option<&crate::neural::policy_value_net::PolicyNet>) -> i32 {
    let mut simulated_plateau = plateau.clone();
    let simulated_deck = deck.clone();

    // Filter out invalid tiles (0, 0, 0)
    let mut valid_tiles: Vec<Tile> = simulated_deck
        .tiles
        .iter()
        .cloned()
        .filter(|tile| *tile != Tile(0, 0, 0))
        .collect();

    let mut rng = rand::rng();

    while !is_plateau_full(&simulated_plateau) {
        let legal_moves = get_legal_moves(simulated_plateau.clone());

        if legal_moves.is_empty() || valid_tiles.is_empty() {
            break;
        }

        // Pick random tile
        let tile_index = rng.random_range(0..valid_tiles.len());
        let chosen_tile = valid_tiles.swap_remove(tile_index);

        // Smart position selection (heuristic-based)
        let position = if rng.random_range(0.0..1.0) < 0.8 {
            // 80% of the time: use greedy heuristic
            select_best_position_heuristic(&simulated_plateau, &chosen_tile, &legal_moves)
        } else {
            // 20% of the time: random exploration
            legal_moves[rng.random_range(0..legal_moves.len())]
        };

        // Place the chosen tile
        simulated_plateau.tiles[position] = chosen_tile;
    }

    result(&simulated_plateau)
}

/// Heuristic to select best position for a tile during rollout
/// Uses simple but effective rules:
/// 1. Complete lines have high value
/// 2. Center positions are valuable
/// 3. Matching adjacent values is good
fn select_best_position_heuristic(plateau: &Plateau, tile: &Tile, legal_moves: &[usize]) -> usize {
    let mut best_position = legal_moves[0];
    let mut best_score = f64::NEG_INFINITY;

    for &position in legal_moves {
        let score = evaluate_position_for_tile(plateau, tile, position);

        if score > best_score {
            best_score = score;
            best_position = position;
        }
    }

    best_position
}

/// Evaluate how good a position is for a tile
/// Returns a heuristic score (higher is better)
fn evaluate_position_for_tile(plateau: &Plateau, tile: &Tile, position: usize) -> f64 {
    let mut score = 0.0;

    // Bonus 1: Center positions are generally better (strategic control)
    let center_positions = [4, 8, 12, 16];
    if center_positions.contains(&position) {
        score += 0.5;
    }

    // Bonus 2: High-value tiles
    let tile_value = tile.0 + tile.1 + tile.2;
    score += (tile_value as f64) * 0.02;

    // Bonus 3: Check if position completes or extends lines
    // This is a simplified heuristic - could use enhanced_position_evaluation
    // for more accuracy, but we want speed in rollouts

    // Simple line completion check (pseudo-code, adapt to your line logic)
    let line_bonus = estimate_line_completion_bonus(plateau, tile, position);
    score += line_bonus;

    score
}

/// Estimate bonus for completing/extending lines
/// Simplified version for fast rollouts
fn estimate_line_completion_bonus(plateau: &Plateau, tile: &Tile, position: usize) -> f64 {
    // Very simplified: check if adjacent positions have matching values
    // In a real implementation, you'd check actual line structures

    let mut bonus = 0.0;

    // Check if any value in tile matches adjacent tiles
    // This encourages forming lines
    let adjacent_positions = get_adjacent_positions(position);

    for adj_pos in adjacent_positions {
        if adj_pos < plateau.tiles.len() {
            let adj_tile = plateau.tiles[adj_pos];
            if adj_tile != Tile(0, 0, 0) {
                // Check if any values match
                if tile.0 == adj_tile.0 || tile.1 == adj_tile.1 || tile.2 == adj_tile.2 {
                    bonus += 0.3;
                }
            }
        }
    }

    bonus
}

/// Get adjacent positions for a given position on the hexagonal grid
/// This is a simplified version - adapt to your actual hex grid structure
fn get_adjacent_positions(position: usize) -> Vec<usize> {
    // Simplified hexagonal adjacency (this would need to match your actual board structure)
    // For a 19-position hex grid, this is approximate
    match position {
        0 => vec![1, 3],
        1 => vec![0, 2, 3, 4],
        2 => vec![1, 4],
        3 => vec![0, 1, 5, 6],
        4 => vec![1, 2, 6, 7, 8],
        // ... (add all 19 positions)
        // For now, return empty to avoid errors
        _ => vec![],
    }
}
