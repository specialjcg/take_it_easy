use rand::{thread_rng, Rng};
use crate::plateau::Plateau;
use crate::test::{choisir_et_placer, create_plateau_empty, create_shuffle_deck,  Tile};
use crate::test::tests::result;

mod test;
mod plateau;

/// Simule 100 parties à partir d'un état donné pour calculer le meilleur score
fn simulate_games(deck: &mut test::Deck, plateau: &mut plateau::Plateau) -> (i32, Plateau) {
    let mut best_score = 0;
    let mut best_plateau = plateau.clone();

    for _ in 0..1000 {
        let mut simulation_deck = deck.clone(); // Clone du deck
        let mut simulation_plateau = plateau.clone(); // Clone du plateau

        // Simule une partie complète avec les clones
        choisir_et_placer(&mut simulation_deck, &mut simulation_plateau);

        // Calcule le score pour cette partie
        let score = result(&simulation_plateau);

        // Met à jour si ce score est le meilleur
        if score > best_score {
            best_score = score;
            best_plateau = simulation_plateau.clone();
        }
    }

    // Retourne le meilleur score trouvé parmi les simulations
    (best_score,best_plateau)
}

/// Trouve la meilleure position pour placer une tuile en simulant 100 parties pour chaque position
fn find_best_position(deck: &mut test::Deck, plateau: &mut plateau::Plateau, tuile: Tile) -> (usize, i32, Plateau) {
    let mut best_score = 0;
    let mut best_position = 0;
    let mut best_plateau_score = plateau.clone();

    for position in 0..plateau.tiles.len() {
        // Vérifie si la case est vide
        if plateau.tiles[position] == Tile(0, 0, 0) {
            // Clone le plateau pour tester ce placement
            let mut temp_plateau = plateau.clone();
            temp_plateau.tiles[position] = tuile;

            // Simule 100 parties avec cette configuration
             let (best_position_score,best_plateau) = simulate_games(deck, &mut temp_plateau);

            // Met à jour si ce score est le meilleur
            if best_position_score > best_score {
                best_score = best_position_score;
                best_position = position;
                best_plateau_score = best_plateau.clone();
                println!("Plateau après placement : {:?}", best_plateau.tiles);
            }
        }
    }

    (best_position, best_score,best_plateau_score)
}
fn generate_tile_image_names(tiles: &[Tile]) -> Vec<String> {
    tiles.iter().map(|tile| {
        format!("../image/{}{}{}.png", tile.0, tile.1, tile.2)
    }).collect()
}
fn main() {
    let mut rng = thread_rng();

    // Initialisation du plateau et du deck
    let mut deck = create_shuffle_deck();
    let mut plateau = create_plateau_empty();

    // Choix d'une tuile aléatoire
    let deck_len = deck.tiles.len();
    let tile_index = rng.gen_range(0..deck_len);
    let tuile_choisie = deck.tiles[tile_index].clone();

    // Suppression de la tuile du deck
    deck = crate::test::remove_tile_from_deck(&deck, &tuile_choisie);

    // Trouve la meilleure position pour la tuile choisie en utilisant le meilleur score
    let (best_position, best_score,best_plateau) = find_best_position(&mut deck, &mut plateau, tuile_choisie);

    // Placement final de la tuile
    plateau.tiles[best_position] = tuile_choisie;

    println!("Tuile choisie : {:?}", tuile_choisie);
    println!("Meilleure position pour la tuile : {}", best_position);
    println!("Meilleur score pour cette position : {}", best_score);
    println!("Plateau après placement : {:?}", plateau.tiles);
    // Generate image filenames based on the tiles in plateau
    let image_names = generate_tile_image_names(&best_plateau.tiles);

    // Output the list of image filenames (you can use this in the frontend)
    println!("List of image filenames: {:?}", image_names);
}
