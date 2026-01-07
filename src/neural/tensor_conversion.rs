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

const HEX_TO_GRID_MAP: [(usize, usize); GRAPH_NODE_COUNT] = [
    (0, 1),
    (0, 2),
    (0, 3),
    (1, 1),
    (1, 2),
    (1, 3),
    (1, 4),
    (2, 0),
    (2, 1),
    (2, 2),
    (2, 3),
    (2, 4),
    (3, 0),
    (3, 1),
    (3, 2),
    (3, 3),
    (4, 1),
    (4, 2),
    (4, 3),
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

// STOCHZERO: Extended channel count for bag awareness
// Set to 8 for legacy encoding, 17 for StochZero with bag awareness
const CHANNELS: usize = 17;  // 8 base + 9 bag features
const GRID_SIZE: usize = 5;

pub fn convert_plateau_to_tensor(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    current_turn: usize,
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

    // Process only the 19 hexagonal cells (plateau.tiles has 19 elements)
    for cell_idx in 0..plateau.tiles.len() {
        // Map LINEAR index to 5×5 grid (same as supervised_trainer)
        let row = cell_idx / GRID_SIZE;
        let col = cell_idx % GRID_SIZE;
        let grid_idx = row * GRID_SIZE + col;

        let plateau_tile = &plateau.tiles[cell_idx];

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
    // Count remaining tiles by value for each direction
    let bag_counts = compute_bag_value_counts(deck);

    // Broadcast bag features to all 19 cells
    for cell_idx in 0..plateau.tiles.len() {
        let row = cell_idx / GRID_SIZE;
        let col = cell_idx % GRID_SIZE;
        let grid_idx = row * GRID_SIZE + col;

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

    Tensor::from_slice(&features).view([1, CHANNELS as i64, GRID_SIZE as i64, GRID_SIZE as i64])
}

/// STOCHZERO: Compute value counts for each direction in the remaining deck
struct BagValueCounts {
    dir1: [f32; 3],  // Counts for [1, 5, 9] normalized /9
    dir2: [f32; 3],  // Counts for [2, 6, 7] normalized /9
    dir3: [f32; 3],  // Counts for [3, 4, 8] normalized /9
}

fn compute_bag_value_counts(deck: &Deck) -> BagValueCounts {
    let mut counts_dir1 = [0u32; 3];  // [1, 5, 9]
    let mut counts_dir2 = [0u32; 3];  // [2, 6, 7]
    let mut counts_dir3 = [0u32; 3];  // [3, 4, 8]

    for tile in &deck.tiles {
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

pub fn convert_plateau_to_graph_features(
    plateau: &Plateau,
    current_turn: usize,
    total_turns: usize,
) -> Tensor {
    let mut features = vec![0f32; GRAPH_NODE_COUNT * 8];
    let orientation_scores = compute_orientation_scores(plateau);
    let turn_normalized = current_turn as f32 / total_turns as f32;

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
