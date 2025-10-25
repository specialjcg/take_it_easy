use crate::game::deck::Deck;
use crate::game::get_legal_moves::get_legal_moves;
use crate::game::plateau::Plateau;
use crate::game::plateau_is_full::is_plateau_full;
use crate::game::tile::Tile;
use crate::scoring::scoring::result;
use rand::Rng;

/// Line definitions from scoring.rs: (positions, length, orientation)
/// Orientation: 0 = horizontal (tile.0), 1 = diagonal1 (tile.1), 2 = diagonal2 (tile.2)
const LINES: &[(&[usize], usize, usize)] = &[
    // Horizontal lines (tile.0)
    (&[0, 1, 2], 3, 0),
    (&[3, 4, 5, 6], 4, 0),
    (&[7, 8, 9, 10, 11], 5, 0),
    (&[12, 13, 14, 15], 4, 0),
    (&[16, 17, 18], 3, 0),
    // Diagonal1 lines (tile.1)
    (&[0, 3, 7], 3, 1),
    (&[1, 4, 8, 12], 4, 1),
    (&[2, 5, 9, 13, 16], 5, 1),
    (&[6, 10, 14, 17], 4, 1),
    (&[11, 15, 18], 3, 1),
    // Diagonal2 lines (tile.2)
    (&[7, 12, 16], 3, 2),
    (&[3, 8, 13, 17], 4, 2),
    (&[0, 4, 9, 14, 18], 5, 2),
    (&[1, 5, 10, 15], 4, 2),
    (&[2, 6, 11], 3, 2),
];

/// Smart rollout using heuristics instead of pure random play
/// This should dramatically improve MCTS evaluation quality
pub fn simulate_games_smart(plateau: Plateau, deck: Deck, _policy_net: Option<&crate::neural::policy_value_net::PolicyNet>) -> i32 {
    let (score, _positions) = simulate_games_smart_with_trace(plateau, deck, _policy_net);
    score
}

/// Smart rollout with RAVE support - returns both score and positions played
/// Used by MCTS to update RAVE statistics
pub fn simulate_games_smart_with_trace(
    plateau: Plateau,
    deck: Deck,
    _policy_net: Option<&crate::neural::policy_value_net::PolicyNet>,
) -> (i32, Vec<usize>) {
    let mut simulated_plateau = plateau.clone();
    let simulated_deck = deck.clone();
    let mut positions_played: Vec<usize> = Vec::new();

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
        positions_played.push(position); // Track for RAVE
    }

    let final_score = result(&simulated_plateau);
    (final_score, positions_played)
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
/// This version uses REAL scoring logic to estimate line completion value
fn evaluate_position_for_tile(plateau: &Plateau, tile: &Tile, position: usize) -> f64 {
    let mut score = 0.0;
    let tile_values = [tile.0, tile.1, tile.2];

    // Evaluate each line that contains this position
    for (line_positions, line_length, orientation) in LINES {
        if !line_positions.contains(&position) {
            continue;
        }

        let tile_value = tile_values[*orientation];
        if tile_value == 0 {
            continue; // Can't score with 0
        }

        // Count how many positions in this line already have matching values
        let mut matching_count = 0;
        let mut has_conflict = false;

        for &pos in *line_positions {
            if pos == position {
                continue; // Skip the position we're evaluating
            }

            let existing_tile = &plateau.tiles[pos];
            if *existing_tile == Tile(0, 0, 0) {
                continue; // Empty position
            }

            let existing_value = match orientation {
                0 => existing_tile.0,
                1 => existing_tile.1,
                2 => existing_tile.2,
                _ => 0,
            };

            if existing_value == tile_value {
                matching_count += 1;
            } else if existing_value != 0 {
                has_conflict = true;
                break; // Line is broken, can't complete
            }
        }

        if has_conflict {
            continue; // This line can't be completed, skip
        }

        // Calculate potential score for this line
        // Score = tile_value × line_length (if completed)
        let potential_score = (tile_value as f64) * (*line_length as f64);

        // Weight by how close we are to completing the line
        let positions_left = *line_length - matching_count - 1; // -1 for current position
        let completion_ratio = (matching_count + 1) as f64 / (*line_length as f64);

        // Exponential bonus for lines close to completion
        // - Line with 4/5 filled: huge bonus
        // - Line with 2/5 filled: moderate bonus
        // - Line with 1/5 filled: small bonus
        let completion_weight = completion_ratio.powi(2); // Quadratic scaling

        score += potential_score * completion_weight;

        // Extra bonus for completing a line RIGHT NOW
        if positions_left == 0 {
            score += potential_score * 2.0; // Triple the value (1x + 2x bonus)
        }
    }

    // Small bonus for center positions (strategic value)
    let center_positions = [4, 8, 9, 12];
    if center_positions.contains(&position) {
        score += 2.0; // Much stronger than before (0.5 → 2.0)
    }

    score
}

/// Get adjacent positions for a given position on the hexagonal grid
/// Based on the actual hexagonal structure from tensor_conversion.rs:
///     0  1  2
///    3  4  5  6
///   7  8  9 10 11
///    12 13 14 15
///      16 17 18
#[allow(dead_code)]
fn get_adjacent_positions(position: usize) -> Vec<usize> {
    match position {
        0 => vec![1, 3],
        1 => vec![0, 2, 4],
        2 => vec![1, 5, 6],
        3 => vec![0, 4, 7],
        4 => vec![1, 3, 5, 8],
        5 => vec![2, 4, 6, 9],
        6 => vec![2, 5, 10, 11],
        7 => vec![3, 8, 12],
        8 => vec![4, 7, 9, 13],
        9 => vec![5, 8, 10, 14],
        10 => vec![6, 9, 11, 15],
        11 => vec![6, 10],
        12 => vec![7, 13, 16],
        13 => vec![8, 12, 14, 17],
        14 => vec![9, 13, 15, 18],
        15 => vec![10, 14],
        16 => vec![12, 17],
        17 => vec![13, 16, 18],
        18 => vec![14, 17],
        _ => vec![],
    }
}
