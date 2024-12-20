use rand::Rng;
use crate::test::{Deck, Plateau, Tile};

pub fn choisir_et_placer(deck: &mut Deck, plateau: &mut Plateau) {
    let mut rng = rand::thread_rng();

    // Répéter jusqu'à ce que le plateau soit plein
    while plateau.tiles.contains(&Tile(0, 0, 0)) {
        // Choisir une tuile aléatoirement dans le deck
        let deck_len = deck.tiles.len();
        if deck_len == 0 {
            break; // Plus de tuiles disponibles
        }
        let tile_index = rng.gen_range(0..deck_len);
        let tuile = deck.tiles.remove(tile_index); // Retirer la tuile du deck

        // Choisir une position aléatoire dans le plateau
        let mut position;
        loop {
            position = rng.gen_range(0..plateau.tiles.len());
            if plateau.tiles[position] == Tile(0, 0, 0) {
                break; // Trouver une case vide
            }
        }

        // Placer la tuile dans le plateau
        plateau.tiles[position] = tuile;
    }
}
