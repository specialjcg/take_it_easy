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
const HEX_TO_GRID_MAP: [(usize, usize); 19] = [
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
const LINE_DEFS: &[(&[usize], usize)] = &[
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

const CHANNELS: usize = 8;
const GRID_SIZE: usize = 5;

pub fn convert_plateau_to_tensor(
    plateau: &Plateau,
    _tile: &Tile,
    _deck: &Deck,
    current_turn: usize,
    total_turns: usize,
) -> Tensor {
    let mut features = vec![0.0f32; CHANNELS * GRID_SIZE * GRID_SIZE];
    let turn_normalized = current_turn as f32 / total_turns as f32;

    let mut orientation_scores = [[0f32; 19]; 3];

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

    for (hex_pos, &(row, col)) in HEX_TO_GRID_MAP.iter().enumerate() {
        let grid_idx = row * GRID_SIZE + col;

        if hex_pos < plateau.tiles.len() {
            let tile = &plateau.tiles[hex_pos];
            features[grid_idx] = (tile.0 as f32 / 10.0).clamp(0.0, 1.0);
            features[GRID_SIZE * GRID_SIZE + grid_idx] = (tile.1 as f32 / 10.0).clamp(0.0, 1.0);
            features[2 * GRID_SIZE * GRID_SIZE + grid_idx] = (tile.2 as f32 / 10.0).clamp(0.0, 1.0);
            features[3 * GRID_SIZE * GRID_SIZE + grid_idx] =
                if *tile == Tile(0, 0, 0) { 0.0 } else { 1.0 };
            features[4 * GRID_SIZE * GRID_SIZE + grid_idx] = turn_normalized;
            features[5 * GRID_SIZE * GRID_SIZE + grid_idx] = orientation_scores[0][hex_pos];
            features[6 * GRID_SIZE * GRID_SIZE + grid_idx] = orientation_scores[1][hex_pos];
            features[7 * GRID_SIZE * GRID_SIZE + grid_idx] = orientation_scores[2][hex_pos];
        }
    }

    Tensor::from_slice(&features).view([1, CHANNELS as i64, GRID_SIZE as i64, GRID_SIZE as i64])
}
