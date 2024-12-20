use crate::test::{Deck, Tile};

pub(crate) fn remove_tile_from_deck(deck: &Deck, tile_to_remove: &Tile) -> Deck {
    // Filtre toutes les tuiles sauf celle à retirer
    let new_tiles: Vec<Tile> = deck
        .tiles
        .iter()
        .filter(|&tile| tile != tile_to_remove) // Conserve uniquement les tuiles différentes
        .cloned() // Copie chaque tuile dans le nouveau vecteur
        .collect();

    Deck { tiles: new_tiles } // Crée un nouveau deck
}
