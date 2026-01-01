//! Optimal Data Generator - Génère des données d'entraînement quasi-optimales
//!
//! Utilise le même beam search que optimal_solver.rs (qui marche à 175 pts)
//! mais sauvegarde les coups joués au format JSON pour l'entraînement.
//!
//! Approche : Stocke un index parent au lieu de cloner l'historique complet

use rand::prelude::*;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fs::File;
use std::io::Write;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::tile::Tile;
use take_it_easy::scoring::scoring::result;

const ALL_TILES: [Tile; 27] = [
    Tile(1, 5, 9),
    Tile(2, 6, 7),
    Tile(3, 4, 8),
    Tile(1, 6, 8),
    Tile(2, 4, 9),
    Tile(3, 5, 7),
    Tile(1, 4, 7),
    Tile(2, 5, 8),
    Tile(3, 6, 9),
    Tile(1, 5, 9),
    Tile(2, 6, 7),
    Tile(3, 4, 8),
    Tile(1, 6, 8),
    Tile(2, 4, 9),
    Tile(3, 5, 7),
    Tile(1, 4, 7),
    Tile(2, 5, 8),
    Tile(3, 6, 9),
    Tile(1, 5, 9),
    Tile(2, 6, 7),
    Tile(3, 4, 8),
    Tile(1, 6, 8),
    Tile(2, 4, 9),
    Tile(3, 5, 7),
    Tile(1, 4, 7),
    Tile(2, 5, 8),
    Tile(3, 6, 9),
];

#[derive(Clone, Serialize, Deserialize)]
struct TrainingExample {
    plateau_state: Vec<i32>,
    tile_played: (i32, i32, i32),
    position_played: usize,
    turn: usize,
    score_after: i32,
}

/// Solution partielle SANS historique (comme optimal_solver.rs)
#[derive(Clone)]
struct PartialSolution {
    plateau: Plateau,
    remaining_tiles: Vec<Tile>,
    score_estimate: f64,
    // Pas de history ici ! On le reconstruira après
}

impl PartialSolution {
    fn new(tiles: Vec<Tile>) -> Self {
        Self {
            plateau: create_plateau_empty(),
            remaining_tiles: tiles,
            score_estimate: 0.0,
        }
    }

    fn evaluate_placement(&self, tile: Tile, position: usize) -> f64 {
        let mut score = 0.0;
        let center_positions = [4, 7, 9, 11, 14];
        if center_positions.contains(&position) {
            score += 3.0;
        }

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

fn get_lines_for_position(position: usize) -> Vec<Vec<usize>> {
    let all_lines = vec![
        vec![0, 5, 10, 15],
        vec![1, 6, 11, 16],
        vec![2, 7, 12, 17],
        vec![3, 8, 13, 18],
        vec![4, 9, 14],
        vec![0, 1, 2, 3, 4],
        vec![5, 6, 7, 8, 9],
        vec![10, 11, 12, 13, 14],
        vec![15, 16, 17, 18],
        vec![0, 4],
        vec![1, 5, 9],
        vec![2, 6, 10, 14],
        vec![3, 7, 11, 15, 18],
        vec![8, 12, 16],
        vec![13, 17],
    ];
    all_lines
        .into_iter()
        .filter(|line| line.contains(&position))
        .collect()
}

fn evaluate_line_potential(plateau: &Plateau, tile: Tile, line: &[usize]) -> f64 {
    let values = [tile.0, tile.1, tile.2];

    let tile_value = match line.len() {
        4 | 5 => {
            if line
                .iter()
                .all(|&p| p < 5 || (5..10).contains(&p) || (10..15).contains(&p) || p >= 15)
            {
                values[1]
            } else if line[0] < line[line.len() - 1] {
                values[0]
            } else {
                values[2]
            }
        }
        _ => {
            if line[0] < line[line.len() - 1] {
                if line[1] - line[0] == 5 {
                    values[0]
                } else {
                    values[2]
                }
            } else {
                values[2]
            }
        }
    };

    let mut filled_count = 0;
    let mut has_conflict = false;
    let line_value_idx = if tile_value == tile.0 {
        0
    } else if tile_value == tile.1 {
        1
    } else {
        2
    };

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
        return 0.0;
    }

    let line_length = line.len();
    let positions_left = line_length - filled_count - 1;
    let completion_ratio = (filled_count + 1) as f64 / line_length as f64;
    let potential_score = (tile_value * line_length as i32) as f64;
    let weight = completion_ratio.powi(2);
    let mut score = potential_score * weight;

    if positions_left == 0 {
        score += potential_score * 3.0;
    }

    score
}

/// Génère une partie avec beam search et reconstruit l'historique APRÈS
fn generate_expert_game(tiles: &[Tile], beam_width: usize) -> (Vec<TrainingExample>, i32) {
    let mut beam: BinaryHeap<PartialSolution> = BinaryHeap::new();
    beam.push(PartialSolution::new(tiles.to_vec()));

    // Beam search SANS historique (comme optimal_solver.rs)
    for _step in 0..19 {
        let mut next_beam: BinaryHeap<PartialSolution> = BinaryHeap::new();

        while let Some(solution) = beam.pop() {
            if solution.remaining_tiles.is_empty() {
                continue;
            }

            for (tile_idx, &tile) in solution.remaining_tiles.iter().enumerate() {
                for position in 0..19 {
                    if solution.plateau.tiles[position] == Tile(0, 0, 0) {
                        let mut new_solution = solution.clone();
                        new_solution.plateau.tiles[position] = tile;
                        new_solution.remaining_tiles.remove(tile_idx);

                        let current_score = result(&new_solution.plateau) as f64;
                        let heuristic_bonus = solution.evaluate_placement(tile, position);
                        new_solution.score_estimate = current_score + heuristic_bonus * 0.1;

                        next_beam.push(new_solution);
                    }
                }
            }
        }

        beam.clear();
        for _ in 0..beam_width.min(next_beam.len()) {
            if let Some(sol) = next_beam.pop() {
                beam.push(sol);
            }
        }
    }

    // Trouver la meilleure solution
    let mut best_score = 0;
    let mut best_plateau: Option<Plateau> = None;

    while let Some(solution) = beam.pop() {
        let score = result(&solution.plateau);
        if score > best_score {
            best_score = score;
            best_plateau = Some(solution.plateau);
        }
    }

    // RECONSTRUIRE l'historique en rejouant avec beam search déterministe
    let mut examples = Vec::new();
    if let Some(final_plateau) = best_plateau {
        examples = reconstruct_history(tiles, final_plateau, beam_width);
    }

    (examples, best_score)
}

/// Reconstruit l'historique en rejouant la partie avec beam width = 1 (greedy)
fn reconstruct_history(
    tiles: &[Tile],
    target_plateau: Plateau,
    _beam_width: usize,
) -> Vec<TrainingExample> {
    let mut examples = Vec::new();
    let mut current_plateau = create_plateau_empty();
    let mut remaining = tiles.to_vec();

    for turn in 0..19 {
        // État du plateau AVANT le coup
        let plateau_state: Vec<i32> = current_plateau
            .tiles
            .iter()
            .flat_map(|t| vec![t.0, t.1, t.2])
            .collect();

        // Trouver le coup qui correspond au plateau cible
        let mut best_tile = Tile(0, 0, 0);
        let mut best_pos = 0;
        let mut found = false;

        for (tile_idx, &tile) in remaining.iter().enumerate() {
            for position in 0..19 {
                if current_plateau.tiles[position] == Tile(0, 0, 0)
                    && target_plateau.tiles[position] == tile
                {
                    // Vérifier que ce coup est compatible
                    let mut test_plateau = current_plateau.clone();
                    test_plateau.tiles[position] = tile;

                    // Si ce coup nous rapproche du plateau cible, c'est le bon
                    let mut matches = true;
                    for i in 0..19 {
                        if test_plateau.tiles[i] != Tile(0, 0, 0)
                            && test_plateau.tiles[i] != target_plateau.tiles[i]
                        {
                            matches = false;
                            break;
                        }
                    }

                    if matches {
                        best_tile = tile;
                        best_pos = position;
                        found = true;
                        break;
                    }
                }
            }
            if found {
                remaining.remove(tile_idx);
                break;
            }
        }

        if !found {
            eprintln!("WARNING: Could not reconstruct history at turn {}", turn);
            break;
        }

        // Jouer le coup
        current_plateau.tiles[best_pos] = best_tile;
        let score_after = result(&current_plateau);

        examples.push(TrainingExample {
            plateau_state,
            tile_played: (best_tile.0, best_tile.1, best_tile.2),
            position_played: best_pos,
            turn,
            score_after,
        });
    }

    examples
}

fn main() {
    use clap::Parser;

    #[derive(Parser, Debug)]
    #[command(name = "optimal_data_generator")]
    #[command(about = "Génère des données d'entraînement avec beam search (FIXED)")]
    struct Args {
        #[arg(short = 'g', long, default_value_t = 50)]
        num_games: usize,

        #[arg(short = 'b', long, default_value_t = 100)]
        beam_width: usize,

        #[arg(short = 'o', long, default_value = "expert_data.json")]
        output: String,

        #[arg(short = 's', long, default_value_t = 2025)]
        seed: u64,
    }

    let args = Args::parse();

    println!("🎓 Optimal Data Generator - FIXED VERSION\n");
    println!("Configuration:");
    println!("  Nombre de parties : {}", args.num_games);
    println!("  Beam width        : {}", args.beam_width);
    println!("  Seed              : {}", args.seed);
    println!("  Output            : {}", args.output);
    println!();

    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut all_examples = Vec::new();
    let mut total_score = 0;

    for game_id in 0..args.num_games {
        let mut tiles: Vec<Tile> = ALL_TILES.to_vec();
        tiles.shuffle(&mut rng);
        tiles.truncate(19);

        print!(
            "Partie {}/{} - Beam search... ",
            game_id + 1,
            args.num_games
        );
        std::io::stdout().flush().unwrap();

        let (examples, score) = generate_expert_game(&tiles, args.beam_width);
        total_score += score;

        println!("Score: {} - {} exemples générés", score, examples.len());
        all_examples.extend(examples);
    }

    let avg_score = total_score as f64 / args.num_games as f64;
    println!("\n📊 Résumé:");
    println!("  Exemples générés : {}", all_examples.len());
    println!("  Score moyen      : {:.2} pts", avg_score);
    println!();

    print!("💾 Sauvegarde des données... ");
    std::io::stdout().flush().unwrap();

    let json = serde_json::to_string_pretty(&all_examples).expect("Erreur sérialisation JSON");
    let mut file = File::create(&args.output).expect("Erreur création fichier");
    file.write_all(json.as_bytes())
        .expect("Erreur écriture fichier");

    println!("✅ Données sauvegardées dans: {}", args.output);
}
