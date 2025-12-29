use crate::game::deck::Deck;
use crate::game::deck_cow::DeckCoW;
use crate::game::tile::Tile;

/// Get all non-empty tiles from a deck
#[allow(dead_code)]
pub fn get_available_tiles(deck: &Deck) -> Vec<Tile> {
    deck.tiles
        .iter()
        .copied()
        .filter(|tile| *tile != Tile(0, 0, 0))
        .collect()
}

// pub fn replace_tile_in_deck(deck: &Deck, tile_to_replace: &Tile) -> Deck {
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
pub fn replace_tile_in_deck(deck: &Deck, tile_to_replace: &Tile) -> Deck {
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

/// CoW version: Replace tile in deck using Copy-on-Write pattern
/// Only clones underlying data when needed for modification
pub fn replace_tile_in_deck_cow(deck_cow: &DeckCoW, tile_to_replace: &Tile) -> DeckCoW {
    let modified = deck_cow.clone_for_modification();
    modified.write(|deck| {
        for tile in &mut deck.tiles {
            if tile == tile_to_replace {
                *tile = Tile(0, 0, 0);
            }
        }
    });
    modified
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_tile_from_deck() {
        // Create a sample deck
        let deck = Deck {
            tiles: vec![Tile(1, 2, 3), Tile(4, 5, 6), Tile(7, 8, 9)],
        };

        // Define the tile to remove
        let tile_to_remove = Tile(4, 5, 6);

        // Remove the tile from the deck
        let updated_deck = replace_tile_in_deck(&deck, &tile_to_remove);

        // Ensure the tile is removed
        assert!(!updated_deck.tiles.contains(&tile_to_remove));

        // Ensure other tiles are still present
        assert!(updated_deck.tiles.contains(&Tile(1, 2, 3)));
        assert!(updated_deck.tiles.contains(&Tile(7, 8, 9)));
    }

    #[test]
    fn test_remove_tile_not_in_deck() {
        // Create a sample deck
        let deck = Deck {
            tiles: vec![Tile(1, 2, 3), Tile(4, 5, 6), Tile(7, 8, 9)],
        };

        // Define a tile that doesn't exist in the deck
        let tile_to_remove = Tile(0, 0, 0);

        // Remove the tile
        let updated_deck = replace_tile_in_deck(&deck, &tile_to_remove);

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
            tiles: vec![Tile(1, 2, 3), Tile(4, 5, 6), Tile(7, 8, 9)],
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
            tiles: vec![Tile(1, 2, 3), Tile(4, 5, 6), Tile(7, 8, 9)],
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
            updated_deck
                .tiles
                .iter()
                .filter(|&&tile| tile == Tile(0, 0, 0))
                .count(),
            0
        );
    }
}
