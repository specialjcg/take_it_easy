pub fn compute_alignment_score(plateau: &Plateau, position: usize, _tile: &Tile) -> f64 {
    let patterns: Vec<(&[usize], Box<dyn Fn(&Tile) -> i32>)> = vec![
        (&[0, 1, 2], Box::new(|t: &Tile| t.0)),
        (&[3, 4, 5, 6], Box::new(|t: &Tile| t.0)),
        (&[7, 8, 9, 10, 11], Box::new(|t: &Tile| t.0)),
        (&[12, 13, 14, 15], Box::new(|t: &Tile| t.0)),
        (&[16, 17, 18], Box::new(|t: &Tile| t.0)),
        (&[0, 3, 7], Box::new(|t: &Tile| t.1)),
        (&[1, 4, 8, 12], Box::new(|t: &Tile| t.1)),
        (&[2, 5, 9, 13, 16], Box::new(|t: &Tile| t.1)),
        (&[6, 10, 14, 17], Box::new(|t: &Tile| t.1)),
        (&[11, 15, 18], Box::new(|t: &Tile| t.1)),
        (&[7, 12, 16], Box::new(|t: &Tile| t.2)),
        (&[3, 8, 13, 17], Box::new(|t: &Tile| t.2)),
        (&[0, 4, 9, 14, 18], Box::new(|t: &Tile| t.2)),
        (&[1, 5, 10, 15], Box::new(|t: &Tile| t.2)),
        (&[2, 6, 11], Box::new(|t: &Tile| t.2)),
    ];

    let mut score = 0.0;

    for (indices, selector) in patterns {
        if indices.contains(&position) {
            let values: Vec<i32> = indices
                .iter()
                .map(|&i| selector(&plateau.tiles[i]))
                .filter(|&v| v != 0)
                .collect();

            if !values.is_empty() {
                let sum = values.iter().sum::<i32>() as f64;
                score += sum / values.len() as f64;
            }
        }
    }

    score
}
use crate::game::plateau::Plateau;
use crate::game::tile::Tile;

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
