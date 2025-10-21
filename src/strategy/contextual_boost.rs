//! Contextual boost system that analyzes line completion potential
//!
//! This module provides intelligent boost calculations based on:
//! - Current plateau state
//! - All 3 bands of each tile (not just band 0)
//! - Line completion proximity
//! - Game phase (early/mid/late)

use crate::game::plateau::Plateau;
use crate::game::tile::Tile;

/// Line definitions: (positions, length, band_index)
/// band_index: 0 = horizontal, 1 = diagonal_type1, 2 = diagonal_type2
const LINES: &[(&[usize], usize, usize)] = &[
    // Horizontal lines (band 0)
    (&[0, 1, 2], 3, 0),
    (&[3, 4, 5, 6], 4, 0),
    (&[7, 8, 9, 10, 11], 5, 0),
    (&[12, 13, 14, 15], 4, 0),
    (&[16, 17, 18], 3, 0),
    // Diagonal type 1 (band 1)
    (&[0, 3, 7], 3, 1),
    (&[1, 4, 8, 12], 4, 1),
    (&[2, 5, 9, 13, 16], 5, 1),
    (&[6, 10, 14, 17], 4, 1),
    (&[11, 15, 18], 3, 1),
    // Diagonal type 2 (band 2)
    (&[7, 12, 16], 3, 2),
    (&[3, 8, 13, 17], 4, 2),
    (&[0, 4, 9, 14, 18], 5, 2),
    (&[1, 5, 10, 15], 4, 2),
    (&[2, 6, 11], 3, 2),
];

/// Analyzes how many tiles in a line already have the target value on the target band
fn count_matching_tiles(
    plateau: &Plateau,
    line_positions: &[usize],
    band_idx: usize,
    target_value: i32,
    exclude_position: usize,
) -> usize {
    line_positions
        .iter()
        .filter(|&&pos| pos != exclude_position)
        .filter(|&&pos| {
            let tile = plateau.tiles[pos];
            if tile == Tile(0, 0, 0) {
                return false; // Empty position
            }
            // Check if this tile has the target value on the correct band
            match band_idx {
                0 => tile.0 == target_value,
                1 => tile.1 == target_value,
                2 => tile.2 == target_value,
                _ => false,
            }
        })
        .count()
}

pub fn calculate_contextual_boost_entropy(
    plateau: &Plateau,
    position: usize,
    tile: &Tile,
    current_turn: usize,
    entropy_factor: f64,
) -> f64 {
    let tile_bands = [tile.0, tile.1, tile.2];
    let mut score = 0.0;

    for (line_positions, length, band_idx) in LINES {
        if !line_positions.contains(&position) {
            continue;
        }

        let target_value = tile_bands[*band_idx];
        if target_value == 0 {
            continue;
        }

        let matches =
            count_matching_tiles(plateau, line_positions, *band_idx, target_value, position);

        let conflicts = line_positions
            .iter()
            .filter(|&&pos| pos != position)
            .filter(|&&pos| {
                let tile = plateau.tiles[pos];
                if tile == Tile(0, 0, 0) {
                    return false;
                }
                let band_value = match band_idx {
                    0 => tile.0,
                    1 => tile.1,
                    2 => tile.2,
                    _ => 0,
                };
                band_value != 0 && band_value != target_value
            })
            .count();

        let filled = line_positions
            .iter()
            .filter(|&&pos| plateau.tiles[pos] != Tile(0, 0, 0))
            .count();

        let completion_ratio = (matches as f64 + 1.0) / (*length as f64);
        let occupancy_ratio = filled as f64 / (*length as f64);
        let conflict_penalty = conflicts as f64 / (*length as f64);

        score += completion_ratio * (1.0 + occupancy_ratio) - conflict_penalty;
    }

    let positional_bonus = match position {
        8 => 1.5,
        9 | 10 => 1.2,
        3 | 4 | 5 | 6 | 12 | 13 | 14 | 15 => 0.9,
        2 | 7 | 11 | 16 => 0.5,
        0 | 1 | 17 | 18 => 0.2,
        _ => 0.0,
    };

    let phase_factor = if current_turn < 6 {
        1.15
    } else if current_turn > 14 {
        0.85
    } else {
        1.0
    };

    let scaled = (score + positional_bonus) * phase_factor;
    let normalized = (scaled / 4.0).tanh().clamp(-1.0, 1.0);
    let entropy_scaled = 0.3 + 0.7 * entropy_factor.clamp(0.0, 1.0);

    normalized * entropy_scaled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::plateau::create_plateau_empty;

    #[test]
    fn test_empty_plateau_has_minimal_boost() {
        let plateau = create_plateau_empty();
        let tile = Tile(9, 5, 1);
        let boost = calculate_contextual_boost_entropy(&plateau, 8, &tile, 5, 1.0);

        assert!(
            boost > 0.0,
            "Empty plateau should still have starting boost"
        );
        assert!(boost <= 1.0, "Boost should be normalized to [-1, 1]");
    }

    #[test]
    fn test_near_complete_line_gets_huge_boost() {
        let mut plateau = create_plateau_empty();

        // Setup: nearly complete horizontal line [7,8,9,10,11] with 9s
        plateau.tiles[7] = Tile(9, 1, 1);
        plateau.tiles[8] = Tile(9, 2, 2);
        plateau.tiles[9] = Tile(9, 3, 3);
        plateau.tiles[10] = Tile(9, 4, 4);
        // Position 11 is empty

        let tile = Tile(9, 5, 5); // Will complete the line!
        let boost = calculate_contextual_boost_entropy(&plateau, 11, &tile, 5, 1.0);

        assert!(boost > 0.6, "Near-complete line should get strong boost");
        assert!(boost <= 1.0, "Boost should remain normalized");
    }

    #[test]
    fn test_all_three_bands_analyzed() {
        let mut plateau = create_plateau_empty();

        // Position 9 is in 3 lines:
        // - Horizontal [7,8,9,10,11] (band 0)
        // - Diagonal [2,5,9,13,16] (band 1)
        // - Diagonal [0,4,9,14,18] (band 2)

        // Setup partial lines
        plateau.tiles[7] = Tile(5, 1, 1);
        plateau.tiles[8] = Tile(5, 2, 2);
        // Position 9 will get Tile(5, 3, 7)

        plateau.tiles[2] = Tile(1, 3, 1);
        plateau.tiles[5] = Tile(2, 3, 2);
        // Position 9 will match band 1 with value 3

        let tile = Tile(5, 3, 7);
        let boost = calculate_contextual_boost_entropy(&plateau, 9, &tile, 5, 1.0);

        assert!(boost > 0.0, "Should analyze multiple bands");
        assert!(boost <= 1.0, "Boost should remain normalized");
    }

    #[test]
    fn test_conflicting_line_no_boost() {
        let mut plateau = create_plateau_empty();

        // Setup conflicting line [0,1,2]
        plateau.tiles[0] = Tile(9, 1, 1);
        plateau.tiles[1] = Tile(5, 2, 2); // Different value on band 0!

        let tile = Tile(9, 3, 3);
        let boost = calculate_contextual_boost_entropy(&plateau, 2, &tile, 5, 1.0);

        assert!(boost < 0.2, "Conflicting line should have minimal boost");
    }
}
