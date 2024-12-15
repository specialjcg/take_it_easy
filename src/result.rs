use crate::test::{Plateau, Tile};

pub fn result(plateau: &Plateau) -> i32 {
    let mut result = 0;

    // Helper function to calculate the score for a matching set of indices
    fn calculate_score(indices: &[usize], multiplier: i32, tiles: &[Tile], selector: fn(&Tile) -> i32) -> i32 {
        let first_value = selector(&tiles[indices[0]]);
        if indices.iter().all(|&i| selector(&tiles[i]) == first_value) {
            return first_value * multiplier;
        }
        0
    }

    // Define the patterns for scoring
    let patterns: Vec<(&[usize], i32, fn(&Tile) -> i32)> = vec![
        (&[0_usize, 1, 2], 3, |tile: &Tile| tile.0), // Horizontal row 1 based on Tile.0
        (&[3, 4, 5, 6], 4, |tile: &Tile| tile.0),   // Horizontal row 2 based on Tile.0
        (&[7, 8, 9, 10, 11], 5, |tile: &Tile| tile.0), // Horizontal row 3 based on Tile.0
        (&[12, 13, 14, 15], 4, |tile: &Tile| tile.0),  // Horizontal row 4 based on Tile.0
        (&[16, 17, 18], 3, |tile: &Tile| tile.0),  // Horizontal row 5 based on Tile.0
        (&[0, 3, 7], 3, |tile: &Tile| tile.1),     // Vertical column 1 based on Tile.1
        (&[1, 4, 8, 12], 4, |tile: &Tile| tile.1), // Vertical column 2 based on Tile.1
        (&[2, 5, 9, 13, 16], 5, |tile: &Tile| tile.1), // Vertical column 3 based on Tile.1
        (&[6, 10, 14, 17], 4, |tile: &Tile| tile.1), // Vertical column 4 based on Tile.1
        (&[11, 15, 18], 3, |tile: &Tile| tile.1),  // Vertical column 5 based on Tile.1
        (&[7, 12, 16], 3, |tile: &Tile| tile.2),   // Diagonal from 7 to 16 based on Tile.2
        (&[3, 8, 13, 17], 4, |tile: &Tile| tile.2), // Diagonal from 3 to 17 based on Tile.2
        (&[0, 4, 9, 14, 18], 5, |tile: &Tile| tile.2), // Diagonal from 0 to 18 based on Tile.2
        (&[1, 5, 10, 15], 4, |tile: &Tile| tile.2), // Diagonal from 1 to 15 based on Tile.2
        (&[2, 6, 11], 3, |tile: &Tile| tile.2),    // Diagonal from 2 to 11 based on Tile.2
    ];

    // Iterate through the patterns and calculate the score
    for (indices, multiplier, selector) in patterns {
        result += calculate_score(indices, multiplier, &plateau.tiles, selector);
    }

    result
}
