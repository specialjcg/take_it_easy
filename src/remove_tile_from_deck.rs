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
pub(crate) fn replace_tile_in_deck(deck: &Deck, tile_to_replace: &Tile) -> Deck {
    // Replace the specified tile with (0, 0, 0)
    let new_tiles: Vec<Tile> = deck
        .tiles
        .iter()
        .map(|tile| {
            if tile == tile_to_replace {
                Tile(0, 0, 0) // Replace the tile
            } else {
                *tile // Keep the original tile
            }
        })
        .collect();

    Deck { tiles: new_tiles } // Return the new deck with replaced tiles
}
