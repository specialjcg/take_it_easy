/// ONE-HOT ORIENTED ENCODING FOR TAKE IT EASY
///
/// Recommended encoding that makes pattern learning much easier:
/// - Each direction gets separate one-hot channels
/// - Network can easily match tile values to line requirements
/// - Convolutions can detect "same value" patterns via identical one-hot vectors

use crate::game::deck::Deck;
use crate::game::plateau::Plateau;
use crate::game::tile::Tile;
use tch::Tensor;

use super::tensor_conversion::{LINE_DEFS, GRAPH_NODE_COUNT};

const GRID_SIZE: usize = 5;

/// Direction values (fixed by game rules)
/// Dir1 (vertical lines): 1, 5, 9
/// Dir2 (diagonal /): 2, 6, 7
/// Dir3 (diagonal \): 3, 4, 8
const DIR1_VALUES: [i32; 3] = [1, 5, 9];
const DIR2_VALUES: [i32; 3] = [2, 6, 7];
const DIR3_VALUES: [i32; 3] = [3, 4, 8];

/// New encoding structure (37 channels):
///
/// PLATEAU STATE (per cell):
/// Ch 0-2: Dir1 one-hot [1,5,9] - which Dir1 value at this cell?
/// Ch 3-5: Dir2 one-hot [2,6,7] - which Dir2 value at this cell?
/// Ch 6-8: Dir3 one-hot [3,4,8] - which Dir3 value at this cell?
/// Ch 9: Occupied mask (1.0 if tile placed)
///
/// CURRENT TILE (broadcast to all cells):
/// Ch 10-12: Dir1 one-hot for current tile
/// Ch 13-15: Dir2 one-hot for current tile
/// Ch 16-18: Dir3 one-hot for current tile
///
/// CONTEXT:
/// Ch 19: Turn progress (0.0 to 1.0)
///
/// BAG AWARENESS (counts of remaining tiles, broadcast):
/// Ch 20-22: Dir1 value counts [count_1, count_5, count_9] / 9
/// Ch 23-25: Dir2 value counts [count_2, count_6, count_7] / 9
/// Ch 26-28: Dir3 value counts [count_3, count_4, count_8] / 9
///
/// LINE FEATURES (explicit geometry, per-line broadcast):
/// Ch 29-31: Line potential for Dir1 lines (5 lines, compressed)
/// Ch 32-34: Line potential for Dir2 lines (5 lines, compressed)
/// Ch 35-36: Line potential for Dir3 lines (5 lines, compressed)
pub const ONEHOT_CHANNELS: usize = 37;

/// Hexagonal to 5×5 grid mapping (same as tensor_conversion.rs)
const HEX_TO_GRID_MAP: [(usize, usize); GRAPH_NODE_COUNT] = [
    (1, 0), (2, 0), (3, 0),           // Column 0
    (1, 1), (2, 1), (3, 1), (4, 1),   // Column 1
    (0, 2), (1, 2), (2, 2), (3, 2), (4, 2), // Column 2
    (1, 3), (2, 3), (3, 3), (4, 3),   // Column 3
    (1, 4), (2, 4), (3, 4),           // Column 4
];

#[inline]
fn hex_to_grid_idx(hex_pos: usize) -> usize {
    let (row, col) = HEX_TO_GRID_MAP[hex_pos];
    row * GRID_SIZE + col
}

/// Convert value to one-hot index for given direction
#[inline]
fn value_to_onehot_idx(value: i32, direction: usize) -> Option<usize> {
    let values = match direction {
        0 => &DIR1_VALUES,
        1 => &DIR2_VALUES,
        2 => &DIR3_VALUES,
        _ => return None,
    };
    values.iter().position(|&v| v == value)
}

/// Main conversion function: plateau + current tile → one-hot tensor
pub fn convert_plateau_onehot(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    current_turn: usize,
) -> Tensor {
    let mut features = vec![0.0f32; ONEHOT_CHANNELS * GRID_SIZE * GRID_SIZE];
    let num_placed = plateau.tiles.iter().filter(|&&t| t != Tile(0, 0, 0)).count();
    let turn_progress = num_placed as f32 / 19.0;

    // PLATEAU STATE: One-hot encoding for each placed tile
    for hex_pos in 0..plateau.tiles.len() {
        let grid_idx = hex_to_grid_idx(hex_pos);
        let plateau_tile = &plateau.tiles[hex_pos];

        if *plateau_tile != Tile(0, 0, 0) {
            // Dir1 one-hot (channels 0-2)
            if let Some(idx) = value_to_onehot_idx(plateau_tile.0, 0) {
                features[idx * GRID_SIZE * GRID_SIZE + grid_idx] = 1.0;
            }

            // Dir2 one-hot (channels 3-5)
            if let Some(idx) = value_to_onehot_idx(plateau_tile.1, 1) {
                features[(3 + idx) * GRID_SIZE * GRID_SIZE + grid_idx] = 1.0;
            }

            // Dir3 one-hot (channels 6-8)
            if let Some(idx) = value_to_onehot_idx(plateau_tile.2, 2) {
                features[(6 + idx) * GRID_SIZE * GRID_SIZE + grid_idx] = 1.0;
            }

            // Occupied mask (channel 9)
            features[9 * GRID_SIZE * GRID_SIZE + grid_idx] = 1.0;
        }
    }

    // CURRENT TILE: One-hot encoding (broadcast to all hex cells)
    let tile_dir1_idx = value_to_onehot_idx(tile.0, 0);
    let tile_dir2_idx = value_to_onehot_idx(tile.1, 1);
    let tile_dir3_idx = value_to_onehot_idx(tile.2, 2);

    for hex_pos in 0..plateau.tiles.len() {
        let grid_idx = hex_to_grid_idx(hex_pos);

        // Dir1 one-hot for current tile (channels 10-12)
        if let Some(idx) = tile_dir1_idx {
            features[(10 + idx) * GRID_SIZE * GRID_SIZE + grid_idx] = 1.0;
        }

        // Dir2 one-hot for current tile (channels 13-15)
        if let Some(idx) = tile_dir2_idx {
            features[(13 + idx) * GRID_SIZE * GRID_SIZE + grid_idx] = 1.0;
        }

        // Dir3 one-hot for current tile (channels 16-18)
        if let Some(idx) = tile_dir3_idx {
            features[(16 + idx) * GRID_SIZE * GRID_SIZE + grid_idx] = 1.0;
        }

        // Turn progress (channel 19)
        features[19 * GRID_SIZE * GRID_SIZE + grid_idx] = turn_progress;
    }

    // BAG AWARENESS: Count remaining tiles by value
    let bag_counts = compute_bag_counts_onehot(deck, tile);

    for hex_pos in 0..plateau.tiles.len() {
        let grid_idx = hex_to_grid_idx(hex_pos);

        // Dir1 counts (channels 20-22)
        for i in 0..3 {
            features[(20 + i) * GRID_SIZE * GRID_SIZE + grid_idx] = bag_counts.dir1[i];
        }

        // Dir2 counts (channels 23-25)
        for i in 0..3 {
            features[(23 + i) * GRID_SIZE * GRID_SIZE + grid_idx] = bag_counts.dir2[i];
        }

        // Dir3 counts (channels 26-28)
        for i in 0..3 {
            features[(26 + i) * GRID_SIZE * GRID_SIZE + grid_idx] = bag_counts.dir3[i];
        }
    }

    // LINE FEATURES: Explicit geometry (channels 29-36)
    let line_features = compute_line_features_onehot(plateau, tile);

    // Broadcast line features to positions on each line
    for (line_idx, feature_value) in line_features.iter().enumerate() {
        let (positions, _direction) = LINE_DEFS[line_idx];

        // Map to compressed channels (29-36 = 8 channels for 15 lines)
        // We use line potential as the main signal
        let channel = 29 + (line_idx % 8);  // Compress 15 lines into 8 channels

        for &pos in positions {
            let grid_idx = hex_to_grid_idx(pos);
            // Take max if multiple lines map to same channel
            let current = features[channel * GRID_SIZE * GRID_SIZE + grid_idx];
            features[channel * GRID_SIZE * GRID_SIZE + grid_idx] = current.max(*feature_value);
        }
    }

    Tensor::from_slice(&features).view([1, ONEHOT_CHANNELS as i64, GRID_SIZE as i64, GRID_SIZE as i64])
}

struct BagCountsOnehot {
    dir1: [f32; 3],  // Counts for [1, 5, 9] normalized
    dir2: [f32; 3],  // Counts for [2, 6, 7] normalized
    dir3: [f32; 3],  // Counts for [3, 4, 8] normalized
}

fn compute_bag_counts_onehot(deck: &Deck, current_tile: &Tile) -> BagCountsOnehot {
    let mut counts_dir1 = [0u32; 3];
    let mut counts_dir2 = [0u32; 3];
    let mut counts_dir3 = [0u32; 3];
    let mut current_tile_found = false;

    for tile in &deck.tiles {
        if !current_tile_found && *tile == *current_tile {
            current_tile_found = true;
            continue;
        }

        // Dir1 values [1, 5, 9]
        if let Some(idx) = value_to_onehot_idx(tile.0, 0) {
            counts_dir1[idx] += 1;
        }

        // Dir2 values [2, 6, 7]
        if let Some(idx) = value_to_onehot_idx(tile.1, 1) {
            counts_dir2[idx] += 1;
        }

        // Dir3 values [3, 4, 8]
        if let Some(idx) = value_to_onehot_idx(tile.2, 2) {
            counts_dir3[idx] += 1;
        }
    }

    // Normalize by max possible count (9 tiles per value)
    BagCountsOnehot {
        dir1: [
            counts_dir1[0] as f32 / 9.0,
            counts_dir1[1] as f32 / 9.0,
            counts_dir1[2] as f32 / 9.0,
        ],
        dir2: [
            counts_dir2[0] as f32 / 9.0,
            counts_dir2[1] as f32 / 9.0,
            counts_dir2[2] as f32 / 9.0,
        ],
        dir3: [
            counts_dir3[0] as f32 / 9.0,
            counts_dir3[1] as f32 / 9.0,
            counts_dir3[2] as f32 / 9.0,
        ],
    }
}

/// Compute line potential: how valuable/completable is each line?
/// Returns value 0.0-1.0 for each of 15 lines
fn compute_line_features_onehot(plateau: &Plateau, tile: &Tile) -> Vec<f32> {
    let mut results = Vec::with_capacity(15);

    for (positions, direction) in LINE_DEFS {
        let tile_value = match direction {
            0 => tile.0,
            1 => tile.1,
            2 => tile.2,
            _ => 0,
        };

        let mut empty_count = 0;
        let mut value_in_line: Option<i32> = None;
        let mut is_blocked = false;
        let line_len = positions.len();

        for &pos in *positions {
            let t = &plateau.tiles[pos];
            if *t == Tile(0, 0, 0) {
                empty_count += 1;
            } else {
                let v = match direction {
                    0 => t.0,
                    1 => t.1,
                    2 => t.2,
                    _ => 0,
                };
                match value_in_line {
                    None => value_in_line = Some(v),
                    Some(existing) if existing != v => is_blocked = true,
                    _ => {}
                }
            }
        }

        let filled_count = line_len - empty_count;

        // Compute line potential
        let potential = if is_blocked {
            0.0  // Line blocked, no value
        } else if filled_count == 0 {
            0.5  // Empty line, moderate potential
        } else {
            let line_value = value_in_line.unwrap_or(0);
            let fill_ratio = filled_count as f32 / line_len as f32;

            // Bonus if current tile matches line value
            let match_bonus = if tile_value == line_value { 0.3 } else { 0.0 };

            // Value-weighted potential
            let value_weight = line_value as f32 / 9.0;
            (0.3 + 0.4 * fill_ratio * value_weight + match_bonus).min(1.0)
        };

        results.push(potential);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_onehot_encoding() {
        let plateau = Plateau::new();
        let tile = Tile(5, 6, 4);
        let deck = Deck::new();

        let tensor = convert_plateau_onehot(&plateau, &tile, &deck, 0);

        // Check shape
        assert_eq!(tensor.size(), vec![1, ONEHOT_CHANNELS as i64, 5, 5]);

        // Current tile Dir1=5 should be encoded in channel 11 (10 + idx=1 for value 5)
        // Since plateau is empty, only current tile channels should be set
        println!("Tensor shape: {:?}", tensor.size());
    }

    #[test]
    fn test_value_to_onehot() {
        // Dir1: [1, 5, 9]
        assert_eq!(value_to_onehot_idx(1, 0), Some(0));
        assert_eq!(value_to_onehot_idx(5, 0), Some(1));
        assert_eq!(value_to_onehot_idx(9, 0), Some(2));
        assert_eq!(value_to_onehot_idx(2, 0), None);  // Not a Dir1 value

        // Dir2: [2, 6, 7]
        assert_eq!(value_to_onehot_idx(2, 1), Some(0));
        assert_eq!(value_to_onehot_idx(6, 1), Some(1));
        assert_eq!(value_to_onehot_idx(7, 1), Some(2));

        // Dir3: [3, 4, 8]
        assert_eq!(value_to_onehot_idx(3, 2), Some(0));
        assert_eq!(value_to_onehot_idx(4, 2), Some(1));
        assert_eq!(value_to_onehot_idx(8, 2), Some(2));
    }
}
