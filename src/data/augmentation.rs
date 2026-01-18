//! Data Augmentation for Take It Easy
//!
//! Implements VALID augmentations using cyclic permutations of tile directions.
//! This is mathematically correct for hexagonal games.

use rand::Rng;

/// Transformation types for data augmentation
/// Uses cyclic permutations of the 3 tile directions (compatible with hexagonal geometry)
#[derive(Debug, Clone, Copy)]
pub enum AugmentTransform {
    Original,      // Tile(a,b,c) - no transformation
    CyclicPerm1,   // Tile(b,c,a) - rotate directions 120°
    CyclicPerm2,   // Tile(c,a,b) - rotate directions 240°
}

impl AugmentTransform {
    /// Get all 3 transformations
    pub fn all() -> [Self; 3] {
        [
            Self::Original,
            Self::CyclicPerm1,
            Self::CyclicPerm2,
        ]
    }

    /// Random transformation
    pub fn random<R: Rng>(rng: &mut R) -> Self {
        let transforms = Self::all();
        transforms[rng.random_range(0..3)]
    }
}

/// Apply cyclic permutation to tile values
fn permute_tile(tile: (i32, i32, i32), transform: AugmentTransform) -> (i32, i32, i32) {
    match transform {
        AugmentTransform::Original => tile,
        AugmentTransform::CyclicPerm1 => (tile.1, tile.2, tile.0), // (a,b,c) → (b,c,a)
        AugmentTransform::CyclicPerm2 => (tile.2, tile.0, tile.1), // (a,b,c) → (c,a,b)
    }
}

/// Decode tile from integer encoding (a*100 + b*10 + c)
fn decode_tile(encoded: i32) -> (i32, i32, i32) {
    if encoded == 0 {
        (0, 0, 0) // Empty tile
    } else {
        let a = encoded / 100;
        let b = (encoded % 100) / 10;
        let c = encoded % 10;
        (a, b, c)
    }
}

/// Encode tile to integer (a*100 + b*10 + c)
fn encode_tile(tile: (i32, i32, i32)) -> i32 {
    if tile == (0, 0, 0) {
        0 // Empty tile
    } else {
        tile.0 * 100 + tile.1 * 10 + tile.2
    }
}

/// Apply cyclic permutation to plateau tiles
fn permute_plateau(plateau_state: &[i32], transform: AugmentTransform) -> Vec<i32> {
    plateau_state
        .iter()
        .map(|&encoded| {
            let tile = decode_tile(encoded);
            let permuted = permute_tile(tile, transform);
            encode_tile(permuted)
        })
        .collect()
}

/// Augment a single training example
///
/// For Take It Easy with hexagonal geometry, we use cyclic permutations of tile directions.
/// This is mathematically valid because the 3 tile directions correspond to the 3 main
/// axes of the hexagonal grid (at 120° from each other).
///
/// Example: Tile(5,6,4) with transform CyclicPerm1 becomes Tile(6,4,5)
pub fn augment_example(
    plateau_state: &[i32],
    tile: (i32, i32, i32),
    position: usize,
    final_score: i32,
    transform: AugmentTransform,
) -> (Vec<i32>, (i32, i32, i32), usize, i32) {
    let new_plateau = permute_plateau(plateau_state, transform);
    let new_tile = permute_tile(tile, transform);

    // Position stays the same - we only permute the tile directions
    (new_plateau, new_tile, position, final_score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cyclic_permutation() {
        let tile = (5, 6, 4);

        // Original
        assert_eq!(permute_tile(tile, AugmentTransform::Original), (5, 6, 4));

        // First permutation: (a,b,c) → (b,c,a)
        assert_eq!(permute_tile(tile, AugmentTransform::CyclicPerm1), (6, 4, 5));

        // Second permutation: (a,b,c) → (c,a,b)
        assert_eq!(permute_tile(tile, AugmentTransform::CyclicPerm2), (4, 5, 6));
    }

    #[test]
    fn test_three_permutations_cycle() {
        let tile = (1, 2, 3);

        // Applying 3 times should return to original
        let perm1 = permute_tile(tile, AugmentTransform::CyclicPerm1);
        let perm2 = permute_tile(perm1, AugmentTransform::CyclicPerm1);
        let perm3 = permute_tile(perm2, AugmentTransform::CyclicPerm1);

        assert_eq!(perm3, tile);
    }

    #[test]
    fn test_augmentation_preserves_score() {
        let plateau = vec![
            123, 456,  // positions 0-1: Tile(1,2,3), Tile(4,5,6)
            0, 0, 0,   // positions 2-4: empty
            789,       // position 5: Tile(7,8,9)
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0  // positions 6-18: empty
        ];
        let tile = (7, 8, 9);
        let position = 5;
        let score = 150;

        for transform in AugmentTransform::all() {
            let (_, _, _, new_score) = augment_example(&plateau, tile, position, score, transform);
            assert_eq!(new_score, score, "Score should be preserved");
        }
    }

    #[test]
    fn test_plateau_permutation() {
        let plateau = vec![
            123,  // Tile(1,2,3) encoded
            456,  // Tile(4,5,6) encoded
            789,  // Tile(7,8,9) encoded
            0,    // Empty tile
            564,  // Tile(5,6,4) encoded
        ];

        let permuted = permute_plateau(&plateau, AugmentTransform::CyclicPerm1);

        // Tile(1,2,3) → Tile(2,3,1) → 231
        assert_eq!(permuted[0], 231);
        // Tile(4,5,6) → Tile(5,6,4) → 564
        assert_eq!(permuted[1], 564);
        // Tile(7,8,9) → Tile(8,9,7) → 897
        assert_eq!(permuted[2], 897);
        // Empty stays empty
        assert_eq!(permuted[3], 0);
        // Tile(5,6,4) → Tile(6,4,5) → 645
        assert_eq!(permuted[4], 645);
    }

    #[test]
    fn test_tile_encoding_decoding() {
        // Test encode/decode round-trip
        let tiles = vec![(1, 6, 3), (5, 6, 4), (9, 2, 4), (0, 0, 0)];

        for tile in tiles {
            let encoded = encode_tile(tile);
            let decoded = decode_tile(encoded);
            assert_eq!(decoded, tile, "Encode/decode should be reversible");
        }

        // Test specific encodings
        assert_eq!(encode_tile((1, 6, 3)), 163);
        assert_eq!(encode_tile((5, 6, 4)), 564);
        assert_eq!(encode_tile((0, 0, 0)), 0);

        assert_eq!(decode_tile(163), (1, 6, 3));
        assert_eq!(decode_tile(564), (5, 6, 4));
        assert_eq!(decode_tile(0), (0, 0, 0));
    }
}
