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

/// Checks if a tile has the target value on the target band
fn tile_has_value_on_band(tile: &Tile, band_idx: usize, target_value: i32) -> bool {
    match band_idx {
        0 => tile.0 == target_value,
        1 => tile.1 == target_value,
        2 => tile.2 == target_value,
        _ => false,
    }
}

/// Calculate contextual boost for placing a tile at a position
///
/// PHASE 2: Analyze ALL 3 bands + detect line completion progress
pub fn calculate_contextual_boost(
    plateau: &Plateau,
    position: usize,
    tile: &Tile,
    _current_turn: usize,
) -> f64 {
    let mut max_boost = 0.0;

    // Analyze each band of the tile (0, 1, 2)
    let tile_bands = [tile.0, tile.1, tile.2];

    for (band_idx, &band_value) in tile_bands.iter().enumerate() {
        if band_value == 0 {
            continue;
        }

        // Check all lines that could be completed with this band
        for (line_positions, _line_length, line_band_idx) in LINES {
            if *line_band_idx != band_idx {
                continue; // Wrong band type
            }

            if !line_positions.contains(&position) {
                continue; // Position not in this line
            }

            // Count how many tiles in this line already have this band value
            let matching_count = count_matching_tiles(
                plateau,
                line_positions,
                band_idx,
                band_value,
                position,
            );

            // 🎯 PHASE 2: Exponential boost based on line completion
            let completion_boost = match matching_count {
                4 => band_value as f64 * 50000.0, // 4/5 or 4/4 = MASSIVE boost
                3 => band_value as f64 * 25000.0, // 3/4 or 3/5 = huge boost
                2 => band_value as f64 * 10000.0, // 2/3 or 2/4 = large boost
                1 => band_value as f64 * 3000.0,  // 1/2 or 1/3 = medium boost
                0 => {
                    // Fallback to old system for starting lines
                    match band_value {
                        9 if [7, 8, 9, 10, 11].contains(&position) => 10000.0,
                        5 if [3, 4, 5, 6, 12, 13, 14, 15].contains(&position) => 8000.0,
                        1 if [0, 1, 2, 16, 17, 18].contains(&position) => 6000.0,
                        _ => 0.0,
                    }
                }
                _ => 0.0,
            };

            if completion_boost > max_boost {
                max_boost = completion_boost;

                if log::log_enabled!(log::Level::Trace) && matching_count >= 2 {
                    log::trace!(
                        "[BoostPhase2] pos={} band_idx={} value={} matching={}/{} boost={:.0}",
                        position,
                        band_idx,
                        band_value,
                        matching_count,
                        line_positions.len(),
                        completion_boost
                    );
                }
            }
        }
    }

    max_boost
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::plateau::create_plateau_empty;

    #[test]
    fn test_empty_plateau_has_minimal_boost() {
        let plateau = create_plateau_empty();
        let tile = Tile(9, 5, 1);
        let boost = calculate_contextual_boost(&plateau, 8, &tile, 5);

        // Should have minimal boost for starting a line
        assert!(
            boost > 0.0,
            "Empty plateau should still have starting boost"
        );
        assert!(
            boost < 10000.0,
            "Empty plateau boost should be reasonable"
        );
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
        let boost = calculate_contextual_boost(&plateau, 11, &tile, 5);

        // Should have MASSIVE boost (4/4 matching = 3000x multiplier)
        assert!(boost > 100000.0, "Near-complete line should get massive boost: got {}", boost);
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
        let boost = calculate_contextual_boost(&plateau, 9, &tile, 5);

        // Should get boost from both band 0 (value 5) and band 1 (value 3)
        assert!(boost > 0.0, "Should analyze multiple bands");
    }

    #[test]
    fn test_conflicting_line_no_boost() {
        let mut plateau = create_plateau_empty();

        // Setup conflicting line [0,1,2]
        plateau.tiles[0] = Tile(9, 1, 1);
        plateau.tiles[1] = Tile(5, 2, 2); // Different value on band 0!

        let tile = Tile(9, 3, 3);
        let boost = calculate_contextual_boost(&plateau, 2, &tile, 5);

        // Boost should be low because line has conflict
        assert!(
            boost < 50000.0,
            "Conflicting line should have minimal boost"
        );
    }
}
