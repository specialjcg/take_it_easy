use crate::game::deck::Deck;
use crate::game::plateau::Plateau;
use crate::game::tile::Tile;
use tch::Tensor;

/// Bronze GNN: Map 19-position hexagonal plateau to 5×5 2D grid preserving spatial structure
///
/// Hexagonal layout (19 positions):
///     0  1  2
///    3  4  5  6
///   7  8  9 10 11
///    12 13 14 15
///      16 17 18
///
/// Mapped to 5×5 grid (padding with zeros for non-hex cells):
///   x  0  1  2  x
///   x  3  4  5  6
///    7  8  9 10 11
///   12 13 14 15  x
///   x 16 17 18  x
pub const GRAPH_NODE_COUNT: usize = 19;

pub const GRAPH_EDGES: &[(usize, usize)] = &[
    (0, 1),
    (1, 2),
    (0, 3),
    (1, 4),
    (2, 5),
    (2, 6),
    (3, 4),
    (4, 5),
    (5, 6),
    (3, 7),
    (4, 8),
    (5, 9),
    (6, 10),
    (6, 11),
    (7, 8),
    (8, 9),
    (9, 10),
    (10, 11),
    (7, 12),
    (8, 13),
    (9, 14),
    (10, 15),
    (12, 13),
    (13, 14),
    (14, 15),
    (12, 16),
    (13, 17),
    (14, 18),
    (16, 17),
    (17, 18),
];

/// Hexagonal plateau layout - VERTICAL columns with 3-4-5-4-3 tiles:
///
///      Col0    Col1    Col2    Col3    Col4
///                       7
///        0      3      8       12      16
///        1      4      9       13      17
///        2      5     10       14      18
///               6     11       15
///
/// Positions 0-2: Column 0 (3 tiles, centered vertically)
/// Positions 3-6: Column 1 (4 tiles)
/// Positions 7-11: Column 2 (5 tiles, full height)
/// Positions 12-15: Column 3 (4 tiles)
/// Positions 16-18: Column 4 (3 tiles, centered vertically)
const HEX_TO_GRID_MAP: [(usize, usize); GRAPH_NODE_COUNT] = [
    // Column 0 (positions 0-2): 3 tiles, rows 1-3
    (1, 0), (2, 0), (3, 0),
    // Column 1 (positions 3-6): 4 tiles, rows 1-4
    (1, 1), (2, 1), (3, 1), (4, 1),
    // Column 2 (positions 7-11): 5 tiles, rows 0-4
    (0, 2), (1, 2), (2, 2), (3, 2), (4, 2),
    // Column 3 (positions 12-15): 4 tiles, rows 1-4
    (1, 3), (2, 3), (3, 3), (4, 3),
    // Column 4 (positions 16-18): 3 tiles, rows 1-3
    (1, 4), (2, 4), (3, 4),
];

/// Line definitions: indices and orientation (0: horizontal, 1: diag1, 2: diag2)
pub const LINE_DEFS: &[(&[usize], usize)] = &[
    (&[0, 1, 2], 0),
    (&[3, 4, 5, 6], 0),
    (&[7, 8, 9, 10, 11], 0),
    (&[12, 13, 14, 15], 0),
    (&[16, 17, 18], 0),
    (&[0, 3, 7], 1),
    (&[1, 4, 8, 12], 1),
    (&[2, 5, 9, 13, 16], 1),
    (&[6, 10, 14, 17], 1),
    (&[11, 15, 18], 1),
    (&[7, 12, 16], 2),
    (&[3, 8, 13, 17], 2),
    (&[0, 4, 9, 14, 18], 2),
    (&[1, 5, 10, 15], 2),
    (&[2, 6, 11], 2),
];

// STOCHZERO V2: Extended with EXPLICIT LINE FEATURES
// - 8 base + 9 bag features + 30 line features = 47 channels
// Line features solve the broken geometry problem where Dir2/Dir3 lines
// are zigzag in the 5x5 grid and can't be detected by convolution
const CHANNELS: usize = 47;  // 17 base + 30 line features
const GRID_SIZE: usize = 5;

// Line features: For each of 15 scoring lines:
// - Ch 17+i*2: Line completion potential (0=blocked, 0.5=partial, 1=complete match)
// - Ch 18+i*2: Current tile compatibility (1 if tile value matches line direction)
// This gives the CNN direct access to line geometry without needing convolution

/// Convert hex position (0-18) to 5×5 grid index using proper hexagonal mapping
/// This preserves spatial relationships so CNN can learn line patterns
#[inline]
fn hex_to_grid_idx(hex_pos: usize) -> usize {
    let (row, col) = HEX_TO_GRID_MAP[hex_pos];
    row * GRID_SIZE + col
}

pub fn convert_plateau_to_tensor(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    _current_turn: usize,
    _total_turns: usize,
) -> Tensor {
    // STOCHZERO: Extended encoding with bag awareness (17 channels):
    // Ch 0-2: Plateau tiles (value1, value2, value3) normalized /9
    // Ch 3: Empty cells mask (1.0 if empty)
    // Ch 4-6: Current tile to place (value1, value2, value3) /9 - BROADCAST to all cells
    // Ch 7: Turn progress (num_placed / 19)
    // Ch 8-10: Dir1 value counts [1, 5, 9] normalized /9 - BROADCAST
    // Ch 11-13: Dir2 value counts [2, 6, 7] normalized /9 - BROADCAST
    // Ch 14-16: Dir3 value counts [3, 4, 8] normalized /9 - BROADCAST

    let mut features = vec![0.0f32; CHANNELS * GRID_SIZE * GRID_SIZE];
    let num_placed = plateau.tiles.iter().filter(|&&t| t != Tile(0, 0, 0)).count();
    let turn_progress = num_placed as f32 / 19.0;

    // Process only the 19 hexagonal cells using CORRECT hexagonal mapping
    for hex_pos in 0..plateau.tiles.len() {
        // Use HEX_TO_GRID_MAP for proper spatial mapping
        let grid_idx = hex_to_grid_idx(hex_pos);

        let plateau_tile = &plateau.tiles[hex_pos];

        if *plateau_tile == Tile(0, 0, 0) {
            // Empty cell
            features[3 * GRID_SIZE * GRID_SIZE + grid_idx] = 1.0;
        } else {
            // Occupied cell - store tile values normalized /9
            features[grid_idx] = plateau_tile.0 as f32 / 9.0;
            features[GRID_SIZE * GRID_SIZE + grid_idx] = plateau_tile.1 as f32 / 9.0;
            features[2 * GRID_SIZE * GRID_SIZE + grid_idx] = plateau_tile.2 as f32 / 9.0;
        }

        // Current tile to place (broadcast to all cells)
        features[4 * GRID_SIZE * GRID_SIZE + grid_idx] = tile.0 as f32 / 9.0;
        features[5 * GRID_SIZE * GRID_SIZE + grid_idx] = tile.1 as f32 / 9.0;
        features[6 * GRID_SIZE * GRID_SIZE + grid_idx] = tile.2 as f32 / 9.0;

        // Turn progress (same for all cells)
        features[7 * GRID_SIZE * GRID_SIZE + grid_idx] = turn_progress;
    }

    // STOCHZERO: Add bag awareness features (broadcast to all cells)
    // Count remaining tiles by value for each direction, EXCLUDING current tile
    let bag_counts = compute_bag_value_counts(deck, tile);

    // Broadcast bag features to all 19 hexagonal cells
    for hex_pos in 0..plateau.tiles.len() {
        let grid_idx = hex_to_grid_idx(hex_pos);

        // Direction 1: values [1, 5, 9]
        features[8 * GRID_SIZE * GRID_SIZE + grid_idx] = bag_counts.dir1[0];
        features[9 * GRID_SIZE * GRID_SIZE + grid_idx] = bag_counts.dir1[1];
        features[10 * GRID_SIZE * GRID_SIZE + grid_idx] = bag_counts.dir1[2];

        // Direction 2: values [2, 6, 7]
        features[11 * GRID_SIZE * GRID_SIZE + grid_idx] = bag_counts.dir2[0];
        features[12 * GRID_SIZE * GRID_SIZE + grid_idx] = bag_counts.dir2[1];
        features[13 * GRID_SIZE * GRID_SIZE + grid_idx] = bag_counts.dir2[2];

        // Direction 3: values [3, 4, 8]
        features[14 * GRID_SIZE * GRID_SIZE + grid_idx] = bag_counts.dir3[0];
        features[15 * GRID_SIZE * GRID_SIZE + grid_idx] = bag_counts.dir3[1];
        features[16 * GRID_SIZE * GRID_SIZE + grid_idx] = bag_counts.dir3[2];
    }

    // EXPLICIT LINE FEATURES (channels 17-46)
    // For each of 15 scoring lines, add 2 features:
    // - Line potential: how valuable is this line? (filled positions × value match)
    // - Tile compatibility: does current tile match this line's direction?
    let line_features = compute_line_features(plateau, tile);

    for (line_idx, (potential, compatibility)) in line_features.iter().enumerate() {
        let channel_potential = 17 + line_idx * 2;
        let channel_compat = 18 + line_idx * 2;

        // Broadcast line features to all positions in that line
        for &pos in LINE_DEFS[line_idx].0 {
            let grid_idx = hex_to_grid_idx(pos);
            features[channel_potential * GRID_SIZE * GRID_SIZE + grid_idx] = *potential;
            features[channel_compat * GRID_SIZE * GRID_SIZE + grid_idx] = *compatibility;
        }
    }

    Tensor::from_slice(&features).view([1, CHANNELS as i64, GRID_SIZE as i64, GRID_SIZE as i64])
}

/// STOCHZERO: Compute value counts for each direction in the remaining deck
/// EXCLUDES the current tile being placed (to match training encoding)
struct BagValueCounts {
    dir1: [f32; 3],  // Counts for [1, 5, 9] normalized /9
    dir2: [f32; 3],  // Counts for [2, 6, 7] normalized /9
    dir3: [f32; 3],  // Counts for [3, 4, 8] normalized /9
}

fn compute_bag_value_counts(deck: &Deck, current_tile: &Tile) -> BagValueCounts {
    let mut counts_dir1 = [0u32; 3];  // [1, 5, 9]
    let mut counts_dir2 = [0u32; 3];  // [2, 6, 7]
    let mut counts_dir3 = [0u32; 3];  // [3, 4, 8]
    let mut current_tile_found = false;

    for tile in &deck.tiles {
        // Skip current tile (match training encoding which excludes it)
        if !current_tile_found && *tile == *current_tile {
            current_tile_found = true;
            continue;
        }

        // Count direction 1 values
        match tile.0 {
            1 => counts_dir1[0] += 1,
            5 => counts_dir1[1] += 1,
            9 => counts_dir1[2] += 1,
            _ => {}
        }

        // Count direction 2 values
        match tile.1 {
            2 => counts_dir2[0] += 1,
            6 => counts_dir2[1] += 1,
            7 => counts_dir2[2] += 1,
            _ => {}
        }

        // Count direction 3 values
        match tile.2 {
            3 => counts_dir3[0] += 1,
            4 => counts_dir3[1] += 1,
            8 => counts_dir3[2] += 1,
            _ => {}
        }
    }

    // Normalize counts /9 (max possible count per value)
    BagValueCounts {
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

/// Compute explicit line features for all 15 scoring lines
/// Returns: Vec of (potential, tile_compatibility) for each line
///
/// - potential: How "complete" and valuable is this line?
///   - 0.0 = blocked (conflicting values placed)
///   - 0.1-0.9 = partial (some matching values)
///   - 1.0 = complete potential (all filled or empty, matching values)
///
/// - tile_compatibility: Does the current tile's value match this line's direction?
///   - 1.0 = tile value matches the dominant value in this line
///   - 0.5 = line has no values yet, tile is compatible
///   - 0.0 = tile value conflicts with line's dominant value
fn compute_line_features(plateau: &Plateau, tile: &Tile) -> Vec<(f32, f32)> {
    let mut results = Vec::with_capacity(15);

    for (positions, direction) in LINE_DEFS {
        // Get the tile value for this direction
        let tile_value = match direction {
            0 => tile.0,
            1 => tile.1,
            2 => tile.2,
            _ => 0,
        };

        // Analyze the line
        let mut empty_count = 0;
        let mut value_counts: [u32; 10] = [0; 10];
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
                if v > 0 && (v as usize) < 10 {
                    value_counts[v as usize] += 1;
                }
            }
        }

        let filled_count = line_len - empty_count;

        // Find dominant value and check if line is blocked
        let (dominant_value, dominant_count) = value_counts
            .iter()
            .enumerate()
            .max_by_key(|(_, &c)| c)
            .map(|(v, &c)| (v as i32, c))
            .unwrap_or((0, 0));

        // Compute line potential
        let potential = if filled_count == 0 {
            // Empty line - full potential, normalized by line value
            0.5
        } else if dominant_count == filled_count as u32 {
            // All filled tiles have same value - line is alive
            // Potential = (filled / total) * (value / 9) for importance weighting
            let fill_ratio = filled_count as f32 / line_len as f32;
            let value_weight = dominant_value as f32 / 9.0;
            0.5 + 0.5 * fill_ratio * value_weight
        } else {
            // Multiple different values - line is blocked
            0.0
        };

        // Compute tile compatibility
        let compatibility = if filled_count == 0 {
            // Empty line - tile is compatible
            0.5
        } else if tile_value == dominant_value {
            // Tile matches the line's value
            1.0
        } else {
            // Tile conflicts with line
            0.0
        };

        results.push((potential, compatibility));
    }

    results
}

pub fn convert_plateau_to_graph_features(
    plateau: &Plateau,
    current_turn: usize,
    total_turns: usize,
) -> Tensor {
    let mut features = vec![0f32; GRAPH_NODE_COUNT * 8];
    let orientation_scores = compute_orientation_scores(plateau);
    let turn_normalized = current_turn as f32 / total_turns as f32;

    #[allow(clippy::needless_range_loop)]
    for node in 0..GRAPH_NODE_COUNT {
        let base = node * 8;
        if node < plateau.tiles.len() {
            let tile = plateau.tiles[node];
            features[base] = (tile.0 as f32 / 10.0).clamp(0.0, 1.0);
            features[base + 1] = (tile.1 as f32 / 10.0).clamp(0.0, 1.0);
            features[base + 2] = (tile.2 as f32 / 10.0).clamp(0.0, 1.0);
            features[base + 3] = if tile == Tile(0, 0, 0) { 0.0 } else { 1.0 };
            features[base + 4] = orientation_scores[0][node];
            features[base + 5] = orientation_scores[1][node];
            features[base + 6] = orientation_scores[2][node];
            features[base + 7] = turn_normalized;
        }
    }

    Tensor::from_slice(&features).view([1, GRAPH_NODE_COUNT as i64, 8])
}

pub fn compute_orientation_scores(plateau: &Plateau) -> [[f32; GRAPH_NODE_COUNT]; 3] {
    let mut orientation_scores = [[0f32; GRAPH_NODE_COUNT]; 3];

    for (positions, orientation) in LINE_DEFS {
        let len = positions.len() as f32;
        if len <= 0.0 {
            continue;
        }

        let mut counts = [0usize; 10];
        let mut filled = 0usize;

        for &pos in *positions {
            let tile = plateau.tiles[pos];
            if tile == Tile(0, 0, 0) {
                continue;
            }
            let value = match orientation {
                0 => tile.0,
                1 => tile.1,
                2 => tile.2,
                _ => 0,
            };
            if value > 0 && (value as usize) < counts.len() {
                counts[value as usize] += 1;
                filled += 1;
            }
        }

        if filled > 0 {
            let max_count = counts.iter().copied().max().unwrap_or(0) as f32;
            let ratio = (max_count / len).clamp(0.0, 1.0);
            for &pos in *positions {
                orientation_scores[*orientation][pos] =
                    orientation_scores[*orientation][pos].max(ratio);
            }
        }
    }

    orientation_scores
}
