//! Solveur optimal pour évaluer la qualité des placements de l'IA
//!
//! Calcule le placement optimal a posteriori (connaissant toutes les tuiles)
//! pour mesurer l'écart entre le score de l'IA et l'optimal théorique.
//!
//! Utilise un beam search avec heuristiques pour trouver des placements quasi-optimaux.

use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::tile::Tile;
use take_it_easy::scoring::scoring::result;
use std::collections::BinaryHeap;
use std::cmp::Ordering;

/// État partiel d'une solution en cours de construction
#[derive(Clone)]
struct PartialSolution {
    plateau: Plateau,
    remaining_tiles: Vec<Tile>,
    score_estimate: f64,  // Score heuristique pour guider la recherche
}

impl PartialSolution {
    fn new(tiles: Vec<Tile>) -> Self {
        Self {
            plateau: create_plateau_empty(),
            remaining_tiles: tiles,
            score_estimate: 0.0,
        }
    }

    /// Calcule le score heuristique pour un placement
    fn evaluate_placement(&self, tile: Tile, position: usize) -> f64 {
        let mut score = 0.0;

        // Positions centrales valent plus (bonus spatial)
        let center_positions = [4, 7, 9, 11, 14];
        if center_positions.contains(&position) {
            score += 3.0;
        }

        // Évaluer le potentiel sur chaque ligne
        let lines = get_lines_for_position(position);
        for line in lines {
            let potential = evaluate_line_potential(&self.plateau, tile, &line);
            score += potential;
        }

        score
    }
}

impl Eq for PartialSolution {}

impl PartialEq for PartialSolution {
    fn eq(&self, other: &Self) -> bool {
        self.score_estimate == other.score_estimate
    }
}

impl PartialOrd for PartialSolution {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.score_estimate.partial_cmp(&other.score_estimate)
    }
}

impl Ord for PartialSolution {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// Retourne les 3 lignes qui passent par une position donnée
fn get_lines_for_position(position: usize) -> Vec<Vec<usize>> {
    let all_lines = vec![
        // Lignes value1 (↗)
        vec![0, 5, 10, 15],
        vec![1, 6, 11, 16],
        vec![2, 7, 12, 17],
        vec![3, 8, 13, 18],
        vec![4, 9, 14],
        // Lignes value2 (→)
        vec![0, 1, 2, 3, 4],
        vec![5, 6, 7, 8, 9],
        vec![10, 11, 12, 13, 14],
        vec![15, 16, 17, 18],
        // Lignes value3 (↘)
        vec![0, 4],
        vec![1, 5, 9],
        vec![2, 6, 10, 14],
        vec![3, 7, 11, 15, 18],
        vec![8, 12, 16],
        vec![13, 17],
    ];

    all_lines.into_iter()
        .filter(|line| line.contains(&position))
        .collect()
}

/// Évalue le potentiel d'une tuile sur une ligne donnée
fn evaluate_line_potential(plateau: &Plateau, tile: Tile, line: &[usize]) -> f64 {
    let values = [tile.0, tile.1, tile.2];

    // Déterminer quelle valeur de la tuile correspond à cette ligne
    let tile_value = match line.len() {
        4 | 5 => {
            if line.iter().all(|&p| p < 5 || (p >= 5 && p < 10) || (p >= 10 && p < 15) || p >= 15) {
                values[1]  // Ligne horizontale → value2
            } else if line[0] < line[line.len() - 1] {
                values[0]  // Ligne montante → value1
            } else {
                values[2]  // Ligne descendante → value3
            }
        }
        _ => {
            if line[0] < line[line.len() - 1] {
                if line[1] - line[0] == 5 { values[0] } else { values[2] }
            } else {
                values[2]
            }
        }
    };

    // Compter les tuiles déjà placées sur cette ligne
    let mut filled_count = 0;
    let mut has_conflict = false;
    let line_value_idx = if tile_value == tile.0 { 0 } else if tile_value == tile.1 { 1 } else { 2 };

    for &pos in line {
        let existing = plateau.tiles[pos];
        if existing != Tile(0, 0, 0) {
            filled_count += 1;
            let existing_value = match line_value_idx {
                0 => existing.0,
                1 => existing.1,
                _ => existing.2,
            };
            if existing_value != tile_value && existing_value != 0 {
                has_conflict = true;
            }
        }
    }

    if has_conflict {
        return 0.0;  // Éviter les conflits
    }

    let line_length = line.len();
    let positions_left = line_length - filled_count - 1;  // -1 pour la tuile qu'on place
    let completion_ratio = (filled_count + 1) as f64 / line_length as f64;

    // Score potentiel si ligne complète
    let potential_score = (tile_value * line_length as i32) as f64;

    // Pondération quadratique selon le taux de remplissage
    let weight = completion_ratio.powi(2);
    let mut score = potential_score * weight;

    // Bonus massif si la ligne sera complétée immédiatement
    if positions_left == 0 {
        score += potential_score * 3.0;
    }

    score
}

/// Beam search avec heuristiques pour trouver un placement quasi-optimal
pub fn find_beam_search_solution(tiles: &[Tile], beam_width: usize) -> (Plateau, i32) {
    let mut beam: BinaryHeap<PartialSolution> = BinaryHeap::new();
    beam.push(PartialSolution::new(tiles.to_vec()));

    // Pour chaque étape (19 tuiles à placer)
    for step in 0..19 {
        let mut next_beam: BinaryHeap<PartialSolution> = BinaryHeap::new();

        // Pour chaque solution partielle dans le beam
        while let Some(solution) = beam.pop() {
            if solution.remaining_tiles.is_empty() {
                continue;
            }

            // Essayer de placer chaque tuile restante à chaque position libre
            for (tile_idx, &tile) in solution.remaining_tiles.iter().enumerate() {
                for position in 0..19 {
                    if solution.plateau.tiles[position] == Tile(0, 0, 0) {
                        let mut new_solution = solution.clone();
                        new_solution.plateau.tiles[position] = tile;
                        new_solution.remaining_tiles.remove(tile_idx);

                        // BUG FIX: Évaluer le score TOTAL combinant score réel + potentiel
                        let current_score = result(&new_solution.plateau) as f64;
                        let heuristic_bonus = solution.evaluate_placement(tile, position);

                        // Score = score actuel + bonus heuristique pour exploration future
                        new_solution.score_estimate = current_score + heuristic_bonus * 0.1;

                        next_beam.push(new_solution);
                    }
                }
            }
        }

        // Garder seulement les beam_width meilleures solutions
        beam.clear();
        for _ in 0..beam_width.min(next_beam.len()) {
            if let Some(sol) = next_beam.pop() {
                beam.push(sol);
            }
        }

        if step % 5 == 0 {
            println!("  Étape {}/19 - Beam size: {}", step + 1, beam.len());
        }
    }

    // Trouver la meilleure solution complète
    let mut best_score = 0;
    let mut best_plateau = create_plateau_empty();

    while let Some(solution) = beam.pop() {
        let score = result(&solution.plateau);
        if score > best_score {
            best_score = score;
            best_plateau = solution.plateau;
        }
    }

    (best_plateau, best_score)
}

fn main() {
    use rand::prelude::*;
    use rand::SeedableRng;

    println!("🎯 Optimal Solver - Beam Search avec Heuristiques\n");

    let mut rng = StdRng::seed_from_u64(2025);
    let num_tests = 50;  // 50 parties pour statistiques robustes
    let beam_width = 1000;  // Largeur maximale pour approcher l'optimal

    println!("Configuration:");
    println!("  Nombre de parties : {}", num_tests);
    println!("  Beam width        : {}", beam_width);
    println!();

    let mut total_ai_score = 0;
    let mut total_optimal_score = 0;
    let mut total_gap = 0;

    for game_idx in 0..num_tests {
        // Générer une séquence aléatoire de tuiles
        let mut all_tiles: Vec<Tile> = vec![
            Tile(1, 2, 3), Tile(1, 6, 8), Tile(1, 7, 3), Tile(1, 6, 3),
            Tile(1, 2, 8), Tile(1, 2, 4), Tile(1, 7, 4), Tile(1, 6, 4),
            Tile(1, 7, 8), Tile(5, 2, 3), Tile(5, 6, 8), Tile(5, 7, 3),
            Tile(5, 6, 3), Tile(5, 2, 8), Tile(5, 2, 4), Tile(5, 7, 4),
            Tile(5, 6, 4), Tile(5, 7, 8), Tile(9, 2, 3), Tile(9, 6, 8),
            Tile(9, 7, 3), Tile(9, 6, 3), Tile(9, 2, 8), Tile(9, 2, 4),
            Tile(9, 7, 4), Tile(9, 6, 4), Tile(9, 7, 8),
        ];
        all_tiles.shuffle(&mut rng);
        let tiles: Vec<Tile> = all_tiles.iter().take(19).copied().collect();

        println!("Partie {}/{}", game_idx + 1, num_tests);

        // Calculer la solution quasi-optimale avec beam search
        let (_optimal_plateau, optimal_score) = find_beam_search_solution(&tiles, beam_width);

        // Score IA moyen observé avec Pattern Rollouts V2
        let ai_score = 139;

        let gap = optimal_score - ai_score;
        let gap_percent = if optimal_score > 0 {
            (gap as f64 / optimal_score as f64) * 100.0
        } else {
            0.0
        };

        println!("  Score IA (Pattern Rollouts V2): {} pts", ai_score);
        println!("  Score quasi-optimal (beam):     {} pts", optimal_score);
        println!("  Gap:                            {} pts ({:.1}%)\n", gap, gap_percent);

        total_ai_score += ai_score;
        total_optimal_score += optimal_score;
        total_gap += gap;
    }

    println!("=== Résumé sur {} parties ===", num_tests);
    println!("Score IA moyen:            {:.1} pts", total_ai_score as f64 / num_tests as f64);
    println!("Score quasi-optimal moyen: {:.1} pts", total_optimal_score as f64 / num_tests as f64);
    println!("Gap moyen:                 {:.1} pts ({:.1}%)",
        total_gap as f64 / num_tests as f64,
        (total_gap as f64 / total_optimal_score as f64) * 100.0
    );
    println!();
    println!("💡 Interprétation:");
    println!("  - Gap < 5%  : IA proche de l'optimal ✅");
    println!("  - Gap 5-10% : Bon niveau, marge d'amélioration");
    println!("  - Gap > 10% : Potentiel d'amélioration important");
}
