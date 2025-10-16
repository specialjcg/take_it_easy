use crate::game::game_state::GameState as BaseGameState;
use crate::game::tile::Tile;
use crate::game::plateau::Plateau;

pub trait GameStateFeatures {
    fn to_tensor_features(&self) -> Vec<f32>;
}

/// Définition des lignes du jeu (15 lignes au total)
/// Format: (indices des positions, multiplicateur, quelle bande de la tuile)
const LINES: &[(&[usize], i32, usize)] = &[
    // Lignes horizontales (5 lignes, bande 0)
    (&[0, 1, 2], 3, 0),
    (&[3, 4, 5, 6], 4, 0),
    (&[7, 8, 9, 10, 11], 5, 0),
    (&[12, 13, 14, 15], 4, 0),
    (&[16, 17, 18], 3, 0),
    // Diagonales type 1 (5 lignes, bande 1)
    (&[0, 3, 7], 3, 1),
    (&[1, 4, 8, 12], 4, 1),
    (&[2, 5, 9, 13, 16], 5, 1),
    (&[6, 10, 14, 17], 4, 1),
    (&[11, 15, 18], 3, 1),
    // Diagonales type 2 (5 lignes, bande 2)
    (&[7, 12, 16], 3, 2),
    (&[3, 8, 13, 17], 4, 2),
    (&[0, 4, 9, 14, 18], 5, 2),
    (&[1, 5, 10, 15], 4, 2),
    (&[2, 6, 11], 3, 2),
];

impl GameStateFeatures for BaseGameState {
    fn to_tensor_features(&self) -> Vec<f32> {
        let mut features = Vec::with_capacity(256);

        // ========================================
        // 1. ÉTAT BRUT DU PLATEAU (19 × 3 = 57)
        // ========================================
        for tile in &self.plateau.tiles {
            features.push(tile.0 as f32 / 9.0);
            features.push(tile.1 as f32 / 9.0);
            features.push(tile.2 as f32 / 9.0);
        }

        // ========================================
        // 2. LIGNES COMPLÈTES (15 × 3 = 45)
        // ========================================
        // Pour chaque ligne, on encode si elle est complète pour les valeurs 1, 5, 9
        for (line_positions, _, band_idx) in LINES {
            let (complete_1, complete_5, complete_9) =
                check_line_completion(&self.plateau, line_positions, *band_idx);
            features.push(if complete_1 { 1.0 } else { 0.0 });
            features.push(if complete_5 { 1.0 } else { 0.0 });
            features.push(if complete_9 { 1.0 } else { 0.0 });
        }

        // ========================================
        // 3. POTENTIEL DE LIGNES (15 × 3 = 45)
        // ========================================
        // Pour chaque ligne, ratio de tuiles placées avec la bonne valeur
        for (line_positions, _, band_idx) in LINES {
            let (potential_1, potential_5, potential_9) =
                check_line_potential(&self.plateau, line_positions, *band_idx);
            features.push(potential_1);
            features.push(potential_5);
            features.push(potential_9);
        }

        // ========================================
        // 4. SCORE PARTIEL ACTUEL (1)
        // ========================================
        let current_score = calculate_current_score(&self.plateau);
        features.push(current_score as f32 / 200.0); // Normalisation

        // ========================================
        // 5. PROGRESSION DU JEU (2)
        // ========================================
        let tiles_placed = self.plateau.tiles.iter()
            .filter(|t| t.0 != 0 || t.1 != 0 || t.2 != 0)
            .count();
        features.push(tiles_placed as f32 / 19.0); // Progression 0-1
        let tiles_remaining = 19 - tiles_placed;
        features.push(tiles_remaining as f32 / 19.0);

        // ========================================
        // 6. DISTRIBUTION DES TUILES DISPONIBLES (27)
        // ========================================
        // Pour chaque valeur (1-9) et chaque bande (0-2), compter les tuiles disponibles
        for band in 0..3 {
            for value in 1..=9 {
                let count = count_deck_tiles(&self.deck, band, value);
                features.push(count as f32 / 3.0); // Max 3 tuiles par valeur
            }
        }

        // ========================================
        // 7. POSITIONS LIBRES STRATÉGIQUES (19)
        // ========================================
        // Marquer les positions libres avec un poids stratégique
        for (pos, tile) in self.plateau.tiles.iter().enumerate() {
            let is_free = tile.0 == 0 && tile.1 == 0 && tile.2 == 0;
            if is_free {
                // Poids basé sur la centralité et les données empiriques
                let strategic_weight = match pos {
                    8 => 1.0,           // Position centrale optimale
                    14 | 2 => 0.9,      // Positions excellentes
                    5 | 11 => 0.8,      // Bonnes positions
                    10 | 13 => 0.7,     // Positions correctes
                    1 | 4 | 6 | 9 | 0 => 0.5, // Positions moyennes
                    12 | 15 | 16 => 0.3,      // Positions faibles
                    7 | 17 => 0.1,            // Positions défavorables
                    3 | 18 => 0.2,            // Autres
                    _ => 0.0,
                };
                features.push(strategic_weight);
            } else {
                features.push(0.0);
            }
        }

        // Total: 57 + 45 + 45 + 1 + 2 + 27 + 19 = 196 features
        // Padding à 256 pour avoir une puissance de 2
        while features.len() < 256 {
            features.push(0.0);
        }

        features
    }
}

/// Vérifie si une ligne est complète pour les valeurs 1, 5, et 9
fn check_line_completion(plateau: &Plateau, line_positions: &[usize], band_idx: usize) -> (bool, bool, bool) {
    let mut complete_1 = true;
    let mut complete_5 = true;
    let mut complete_9 = true;

    for &pos in line_positions {
        let tile = &plateau.tiles[pos];
        let value = match band_idx {
            0 => tile.0,
            1 => tile.1,
            2 => tile.2,
            _ => 0,
        };

        if value != 1 {
            complete_1 = false;
        }
        if value != 5 {
            complete_5 = false;
        }
        if value != 9 {
            complete_9 = false;
        }
    }

    (complete_1, complete_5, complete_9)
}

/// Calcule le potentiel d'une ligne (ratio de tuiles placées avec chaque valeur)
fn check_line_potential(plateau: &Plateau, line_positions: &[usize], band_idx: usize) -> (f32, f32, f32) {
    let mut count_1 = 0;
    let mut count_5 = 0;
    let mut count_9 = 0;
    let mut count_empty = 0;

    for &pos in line_positions {
        let tile = &plateau.tiles[pos];
        if tile.0 == 0 && tile.1 == 0 && tile.2 == 0 {
            count_empty += 1;
            continue;
        }

        let value = match band_idx {
            0 => tile.0,
            1 => tile.1,
            2 => tile.2,
            _ => 0,
        };

        match value {
            1 => count_1 += 1,
            5 => count_5 += 1,
            9 => count_9 += 1,
            _ => {}
        }
    }

    let total = line_positions.len() as f32;
    (
        count_1 as f32 / total,
        count_5 as f32 / total,
        count_9 as f32 / total,
    )
}

/// Calcule le score actuel du plateau (sans pénalités)
fn calculate_current_score(plateau: &Plateau) -> i32 {
    let mut score = 0;

    for (line_positions, multiplier, band_idx) in LINES {
        let first_value = match band_idx {
            0 => plateau.tiles[line_positions[0]].0,
            1 => plateau.tiles[line_positions[0]].1,
            2 => plateau.tiles[line_positions[0]].2,
            _ => 0,
        };

        if first_value == 0 {
            continue;
        }

        let all_match = line_positions.iter().all(|&pos| {
            let tile = &plateau.tiles[pos];
            let value = match band_idx {
                0 => tile.0,
                1 => tile.1,
                2 => tile.2,
                _ => 0,
            };
            value == first_value
        });

        if all_match {
            score += first_value * multiplier;
        }
    }

    score
}

/// Compte les tuiles disponibles dans le deck pour une bande et une valeur données
fn count_deck_tiles(deck: &crate::game::deck::Deck, band: usize, value: i32) -> usize {
    // Note: deck.tiles est pub(crate), accessible depuis le même crate
    deck.tiles.iter().filter(|tile| {
        let tile_value = match band {
            0 => tile.0,
            1 => tile.1,
            2 => tile.2,
            _ => 0,
        };
        tile_value == value
    }).count()
}
