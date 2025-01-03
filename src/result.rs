use crate::test::{Plateau, Tile};

pub fn result(plateau: &Plateau) -> i32 {
    let mut result = 0;

    // Inline the logic of calculate_score
    let patterns: Vec<(&[usize], i32, Box<dyn Fn(&Tile) -> i32>)> = vec![
        (&[0, 1, 2][..], 3, Box::new(|tile: &Tile| tile.0)),
        (&[3, 4, 5, 6][..], 4, Box::new(|tile: &Tile| tile.0)),
        (&[7, 8, 9, 10, 11][..], 5, Box::new(|tile: &Tile| tile.0)),
        (&[12, 13, 14, 15][..], 4, Box::new(|tile: &Tile| tile.0)),
        (&[16, 17, 18][..], 3, Box::new(|tile: &Tile| tile.0)),
        (&[0, 3, 7][..], 3, Box::new(|tile: &Tile| tile.1)),
        (&[1, 4, 8, 12][..], 4, Box::new(|tile: &Tile| tile.1)),
        (&[2, 5, 9, 13, 16][..], 5, Box::new(|tile: &Tile| tile.1)),
        (&[6, 10, 14, 17][..], 4, Box::new(|tile: &Tile| tile.1)),
        (&[11, 15, 18][..], 3, Box::new(|tile: &Tile| tile.1)),
        (&[7, 12, 16][..], 3, Box::new(|tile: &Tile| tile.2)),
        (&[3, 8, 13, 17][..], 4, Box::new(|tile: &Tile| tile.2)),
        (&[0, 4, 9, 14, 18][..], 5, Box::new(|tile: &Tile| tile.2)),
        (&[1, 5, 10, 15][..], 4, Box::new(|tile: &Tile| tile.2)),
        (&[2, 6, 11][..], 3, Box::new(|tile: &Tile| tile.2)),
    ];

    for (indices, multiplier, selector) in patterns {
        let first_value = selector(&plateau.tiles[indices[0]]);
        if indices.iter().all(|&i| selector(&plateau.tiles[i]) == first_value) {
            result += first_value * multiplier;
        }
    }

    result
}
