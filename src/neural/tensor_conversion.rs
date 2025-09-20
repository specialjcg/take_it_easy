use tch::Tensor;
use crate::game::deck::Deck;
use crate::game::plateau::Plateau;
use crate::game::tile::Tile;

pub fn convert_plateau_to_tensor(
    plateau: &Plateau,
    _tile: &Tile, // Add underscore prefix
    _deck: &Deck, // Add underscore prefix
    current_turn: usize,
    total_turns: usize,
) -> Tensor {
    let mut features = vec![0.0; 5 * 47]; // 5 channels

    // Channel 1-3: Plateau (only use plateau data, not tile/deck)
    for (i, t) in plateau.tiles.iter().enumerate() {
        if i < 19 {
            features[i] = (t.0 as f32 / 10.0).clamp(0.0, 1.0);
            features[47 + i] = (t.1 as f32 / 10.0).clamp(0.0, 1.0);
            features[2 * 47 + i] = (t.2 as f32 / 10.0).clamp(0.0, 1.0);
        }
    }

    // Channel 4: Score Potential for each position
    let potential_scores = compute_potential_scores(plateau);
    for i in 0..19 {
        features[3 * 47 + i] = potential_scores[i];
    }

    // Channel 5: Current Turn
    let turn_normalized = current_turn as f32 / total_turns as f32;
    for i in 0..19 {
        features[4 * 47 + i] = turn_normalized;
    }

    Tensor::from_slice(&features).view([1, 5, 47, 1])
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

