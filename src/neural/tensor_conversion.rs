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
/// Mapped to 5×5 grid (padding with -1 for empty cells):
///   -1  0  1  2 -1
///   -1  3  4  5  6
///    7  8  9 10 11
///   12 13 14 15 -1
///   -1 16 17 18 -1
const HEX_TO_GRID_MAP: [(usize, usize); 19] = [
    // Row 0: positions 0,1,2
    (0, 1), (0, 2), (0, 3),
    // Row 1: positions 3,4,5,6
    (1, 1), (1, 2), (1, 3), (1, 4),
    // Row 2: positions 7,8,9,10,11
    (2, 0), (2, 1), (2, 2), (2, 3), (2, 4),
    // Row 3: positions 12,13,14,15
    (3, 0), (3, 1), (3, 2), (3, 3),
    // Row 4: positions 16,17,18
    (4, 1), (4, 2), (4, 3),
];

pub fn convert_plateau_to_tensor(
    plateau: &Plateau,
    _tile: &Tile,
    _deck: &Deck,
    current_turn: usize,
    total_turns: usize,
) -> Tensor {
    // 5 channels × 5 rows × 5 cols = 125 features
    let mut features = vec![-1.0f32; 5 * 5 * 5]; // Initialize with -1 for padding

    let potential_scores = compute_potential_scores(plateau);
    let turn_normalized = current_turn as f32 / total_turns as f32;

    // Map each of the 19 hexagonal positions to the 5×5 grid
    for (hex_pos, &(row, col)) in HEX_TO_GRID_MAP.iter().enumerate() {
        let grid_idx = row * 5 + col;

        if hex_pos < plateau.tiles.len() {
            let tile = &plateau.tiles[hex_pos];

            // Channel 0: Band 0 (red band)
            features[grid_idx] = (tile.0 as f32 / 10.0).clamp(0.0, 1.0);

            // Channel 1: Band 1 (green band)
            features[25 + grid_idx] = (tile.1 as f32 / 10.0).clamp(0.0, 1.0);

            // Channel 2: Band 2 (blue band)
            features[50 + grid_idx] = (tile.2 as f32 / 10.0).clamp(0.0, 1.0);

            // Channel 3: Score Potential
            features[75 + grid_idx] = if hex_pos < potential_scores.len() {
                potential_scores[hex_pos]
            } else {
                0.0
            };

            // Channel 4: Current Turn (normalized)
            features[100 + grid_idx] = turn_normalized;
        }
    }

    // Reshape to [batch, channels, height, width] = [1, 5, 5, 5]
    Tensor::from_slice(&features).view([1, 5, 5, 5])
}
// Type alias for complex pattern type
type PatternTuple = (&'static [usize], i32, Box<dyn Fn(&Tile) -> i32>);

fn compute_potential_scores(plateau: &Plateau) -> Vec<f32> {
    let mut scores = vec![0.0; 19]; // Potential score for each position

    let patterns: Vec<PatternTuple> = vec![
        (&[0, 1, 2], 3, Box::new(|tile: &Tile| tile.0)),
        (&[3, 4, 5, 6], 4, Box::new(|tile: &Tile| tile.0)),
        (&[7, 8, 9, 10, 11], 5, Box::new(|tile: &Tile| tile.0)),
        (&[12, 13, 14, 15], 4, Box::new(|tile: &Tile| tile.0)),
        (&[16, 17, 18], 3, Box::new(|tile: &Tile| tile.0)),
        (&[0, 3, 7], 3, Box::new(|tile: &Tile| tile.1)),
        (&[1, 4, 8, 12], 4, Box::new(|tile: &Tile| tile.1)),
        (&[2, 5, 9, 13, 16], 5, Box::new(|tile: &Tile| tile.1)),
        (&[6, 10, 14, 17], 4, Box::new(|tile: &Tile| tile.1)),
        (&[11, 15, 18], 3, Box::new(|tile: &Tile| tile.1)),
        (&[7, 12, 16], 3, Box::new(|tile: &Tile| tile.2)),
        (&[3, 8, 13, 17], 4, Box::new(|tile: &Tile| tile.2)),
        (&[0, 4, 9, 14, 18], 5, Box::new(|tile: &Tile| tile.2)),
        (&[1, 5, 10, 15], 4, Box::new(|tile: &Tile| tile.2)),
        (&[2, 6, 11], 3, Box::new(|tile: &Tile| tile.2)),
    ];

    for (indices, multiplier, selector) in &patterns {
        let mut filled_values = Vec::new();
        let mut empty_positions = Vec::new();

        for &pos in *indices {
            if plateau.tiles[pos] == Tile(0, 0, 0) {
                empty_positions.push(pos);
            } else {
                filled_values.push(selector(&plateau.tiles[pos]) as f32);
            }
        }

        // If at least one tile is placed in the pattern
        if !filled_values.is_empty() {
            let avg_filled_value = filled_values.iter().sum::<f32>() / filled_values.len() as f32;
            let potential_score = avg_filled_value * (*multiplier as f32);

            for &pos in empty_positions.iter() {
                scores[pos] += potential_score / empty_positions.len() as f32; // Distribute potential score
            }
        }
    }

    scores
}
