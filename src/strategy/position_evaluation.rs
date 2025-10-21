use crate::game::plateau::Plateau;
use crate::game::tile::Tile;
use crate::scoring::scoring::compute_alignment_score;

/// Line definitions: (positions, band_index)
const LINES: &[(&[usize], usize)] = &[
    // Horizontal lines (band 0)
    (&[0, 1, 2], 0),
    (&[3, 4, 5, 6], 0),
    (&[7, 8, 9, 10, 11], 0),
    (&[12, 13, 14, 15], 0),
    (&[16, 17, 18], 0),
    // Diagonal type 1 (band 1)
    (&[0, 3, 7], 1),
    (&[1, 4, 8, 12], 1),
    (&[2, 5, 9, 13, 16], 1),
    (&[6, 10, 14, 17], 1),
    (&[11, 15, 18], 1),
    // Diagonal type 2 (band 2)
    (&[7, 12, 16], 2),
    (&[3, 8, 13, 17], 2),
    (&[0, 4, 9, 14, 18], 2),
    (&[1, 5, 10, 15], 2),
    (&[2, 6, 11], 2),
];

// Version simplifiée qui se concentre sur les positions stratégiques
pub fn calculate_line_completion_bonus(_plateau: &Plateau, position: usize, tile: &Tile) -> f64 {
    let mut bonus = 0.0;

    // Bonus basé sur les positions stratégiques identifiées dans tes données
    bonus += match position {
        8 => 5.0,                 // Position 8: 150.6 moyenne - excellente
        14 => 4.0,                // Position 14: 147.7 moyenne - très bonne
        2 => 4.0,                 // Position 2: 147.1 moyenne - très bonne
        5 => 3.0,                 // Position 5: 143.6 moyenne - bonne
        11 => 3.0,                // Position 11: 142.9 moyenne - bonne
        10 => 2.0,                // Position 10: 140.8 moyenne - correcte
        13 => 2.0,                // Position 13: 140.2 moyenne - correcte
        1 | 4 | 6 | 9 | 0 => 1.0, // Positions moyennes
        12 | 15 | 16 => 0.5,      // Positions plus faibles
        7 | 17 => 0.0,            // Positions les plus faibles
        _ => 0.0,
    };

    // Bonus pour les valeurs de tuiles élevées (plus de points potentiels)
    let tile_value_bonus = ((tile.0 + tile.1) as f64) * 0.1;
    bonus += tile_value_bonus;

    // Bonus pour la cohérence des couleurs/formes
    if tile.0 == tile.1 {
        bonus += 1.0; // Tuiles avec même couleur et forme
    }

    // Bonus central légèrement plus complexe
    let row = position / 3;
    let col = position % 3;
    if (1..=4).contains(&row) && col >= 1 && col <= 1 {
        bonus += 2.0; // Zone centrale du plateau
    }

    bonus
}

// ============================================================================
// ALTERNATIVE PLUS SIMPLE (Si la version ci-dessus pose encore problème)
// ============================================================================

// Si vous préférez une version plus simple, utilisez celle-ci:

pub fn enhanced_position_evaluation(
    plateau: &Plateau,
    position: usize,
    tile: &Tile,
    current_turn: usize,
) -> f64 {
    // Score de base alignement (votre fonction existante)
    let alignment_score = compute_alignment_score(plateau, position, tile);

    // Bonus pour positions centrales stratégiques en début de partie
    let position_bonus = if current_turn < 8 {
        match position {
            7..=11 => 5.0,                        // Ligne centrale - critique
            4 | 5 | 6 | 12 | 13 | 14 | 15 => 3.0, // Positions stratégiques
            _ => 0.0,
        }
    } else {
        0.0 // En fin de partie, seul l'alignement compte
    };

    // Malus pour positions coins/bords si début de partie
    let position_malus = if current_turn < 5 {
        match position {
            0 | 2 | 16 | 18 => -2.0, // Coins - à éviter en début
            1 | 17 => -1.0,          // Bords
            _ => 0.0,
        }
    } else {
        0.0
    };

    // Bonus pour complétion de lignes
    let completion_bonus = calculate_line_completion_bonus(plateau, position, tile);

    // 🎯 Keep it simple: let MCTS + ValueNet handle complex patterns
    // Multi-line potential is useful but empirical bonuses were misleading
    let multi_line_bonus = calculate_multi_line_potential(position, tile);

    alignment_score + position_bonus + position_malus + completion_bonus + multi_line_bonus
}

/// Calculate bonus for positions that participate in multiple lines
fn calculate_multi_line_potential(position: usize, tile: &Tile) -> f64 {
    let tile_bands = [tile.0, tile.1, tile.2];
    let mut line_count = 0;

    // Count how many lines this position belongs to
    for (line_positions, band_idx) in LINES {
        if !line_positions.contains(&position) {
            continue;
        }

        // Check if tile has a value on this band
        let band_value = tile_bands[*band_idx];
        if band_value > 0 {
            line_count += 1;
        }
    }

    // More lines = more flexibility and scoring potential
    match line_count {
        3 => 3.0, // Excellent: 3 lines
        2 => 1.5, // Good: 2 lines
        1 => 0.0, // Normal: 1 line
        _ => 0.0,
    }
}
