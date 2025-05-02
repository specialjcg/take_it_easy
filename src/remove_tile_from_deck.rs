use crate::test::{Deck, Tile};


// pub(crate) fn replace_tile_in_deck(deck: &Deck, tile_to_replace: &Tile) -> Deck {
//     // Replace the specified tile with (0, 0, 0)
//     let new_tiles: Vec<Tile> = deck
//         .tiles
//         .iter()
//         .map(|tile| {
//             if tile == tile_to_replace {
//                 Tile(0, 0, 0) // Replace the tile
//             } else {
//                 *tile // Keep the original tile
//             }
//         })
//         .collect();
//
//     Deck { tiles: new_tiles } // Return the new deck with replaced tiles
// }
pub(crate) fn replace_tile_in_deck(deck: &Deck, tile_to_remove: &Tile) -> Deck {
    let mut found = false;

    let new_tiles: Vec<Tile> = deck
        .tiles
        .iter()
        .cloned()
        .filter(|tile| {
            if !found && tile == tile_to_remove {
                found = true; // remove only the first match
                false
            } else {
                true
            }
        })
        .collect();

    Deck { tiles: new_tiles }
}

pub fn remove_tile_from_deck(deck: &Deck, tile_to_remove: &Tile) -> Deck {
    // Filtre toutes les tuiles sauf celle à retirer
    let new_tiles: Vec<Tile> = deck
        .tiles
        .iter()
        .filter(|&tile| tile != tile_to_remove) // Conserve uniquement les tuiles différentes
        .cloned() // Copie chaque tuile dans le nouveau vecteur
        .collect();

    Deck { tiles: new_tiles } // Crée un nouveau deck
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_tile_from_deck() {
        // Create a sample deck
        let deck = Deck {
            tiles: vec![
                Tile(1, 2, 3),
                Tile(4, 5, 6),
                Tile(7, 8, 9),
            ],
        };

        // Define the tile to remove
        let tile_to_remove = Tile(4, 5, 6);

        // Remove the tile from the deck
        let updated_deck = remove_tile_from_deck(&deck, &tile_to_remove);

        // Ensure the tile is removed
        assert_eq!(updated_deck.tiles.len(), 2);
        assert!(!updated_deck.tiles.contains(&tile_to_remove));

        // Ensure other tiles are still present
        assert!(updated_deck.tiles.contains(&Tile(1, 2, 3)));
        assert!(updated_deck.tiles.contains(&Tile(7, 8, 9)));
    }

    #[test]
    fn test_remove_tile_not_in_deck() {
        // Create a sample deck
        let deck = Deck {
            tiles: vec![
                Tile(1, 2, 3),
                Tile(4, 5, 6),
                Tile(7, 8, 9),
            ],
        };

        // Define a tile that doesn't exist in the deck
        let tile_to_remove = Tile(0, 0, 0);

        // Remove the tile
        let updated_deck = remove_tile_from_deck(&deck, &tile_to_remove);

        // Ensure the deck size remains unchanged
        assert_eq!(updated_deck.tiles.len(), 3);

        // Ensure all original tiles are still present
        assert!(updated_deck.tiles.contains(&Tile(1, 2, 3)));
        assert!(updated_deck.tiles.contains(&Tile(4, 5, 6)));
        assert!(updated_deck.tiles.contains(&Tile(7, 8, 9)));
    }

    #[test]
    fn test_replace_tile_in_deck() {
        // Create a sample deck
        let deck = Deck {
            tiles: vec![
                Tile(1, 2, 3),
                Tile(4, 5, 6),
                Tile(7, 8, 9),
            ],
        };

        // Define the tile to replace
        let tile_to_replace = Tile(4, 5, 6);

        // Replace the tile in the deck
        let updated_deck = replace_tile_in_deck(&deck, &tile_to_replace);

        // Ensure the tile is replaced with (0, 0, 0)
        assert!(updated_deck.tiles.contains(&Tile(0, 0, 0)));

        // Ensure other tiles are still present
        assert!(updated_deck.tiles.contains(&Tile(1, 2, 3)));
        assert!(updated_deck.tiles.contains(&Tile(7, 8, 9)));

        // Ensure the replaced tile is no longer present
        assert!(!updated_deck.tiles.contains(&tile_to_replace));
    }

    #[test]
    fn test_replace_tile_not_in_deck() {
        // Create a sample deck
        let deck = Deck {
            tiles: vec![
                Tile(1, 2, 3),
                Tile(4, 5, 6),
                Tile(7, 8, 9),
            ],
        };

        // Define a tile that doesn't exist in the deck
        let tile_to_replace = Tile(0, 0, 0);

        // Replace the tile
        let updated_deck = replace_tile_in_deck(&deck, &tile_to_replace);

        // Ensure the deck size remains unchanged
        assert_eq!(updated_deck.tiles.len(), 3);

        // Ensure all original tiles are still present
        assert!(updated_deck.tiles.contains(&Tile(1, 2, 3)));
        assert!(updated_deck.tiles.contains(&Tile(4, 5, 6)));
        assert!(updated_deck.tiles.contains(&Tile(7, 8, 9)));

        // Ensure (0, 0, 0) is not added unless it replaces something
        assert_eq!(
            updated_deck.tiles.iter().filter(|&&tile| tile == Tile(0, 0, 0)).count(),
            0
        );
    }
}
