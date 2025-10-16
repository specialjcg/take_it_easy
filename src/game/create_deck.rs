pub(crate) use crate::game::deck::Deck;
use crate::game::tile::Tile;

pub fn create_deck() -> Deck {
    let tiles = vec![
        new_tiles(1, 2, 3),
        new_tiles(1, 6, 8),
        new_tiles(1, 7, 3),
        new_tiles(1, 6, 3),
        new_tiles(1, 2, 8),
        new_tiles(1, 2, 4),
        new_tiles(1, 7, 4),
        new_tiles(1, 6, 4),
        new_tiles(1, 7, 8),
        new_tiles(5, 2, 3),
        new_tiles(5, 6, 8),
        new_tiles(5, 7, 3),
        new_tiles(5, 6, 3),
        new_tiles(5, 2, 8),
        new_tiles(5, 2, 4),
        new_tiles(5, 7, 4),
        new_tiles(5, 6, 4),
        new_tiles(5, 7, 8),
        new_tiles(9, 2, 3),
        new_tiles(9, 6, 8),
        new_tiles(9, 7, 3),
        new_tiles(9, 6, 3),
        new_tiles(9, 2, 8),
        new_tiles(9, 2, 4),
        new_tiles(9, 7, 4),
        new_tiles(9, 6, 4),
        new_tiles(9, 7, 8),
    ];

    Deck { tiles }
}

pub(crate) fn new_tiles(x: i32, y: i32, z: i32) -> Tile {
    Tile(x, y, z)
}
#[cfg(test)]
mod tests {
    use crate::game::create_deck::create_deck;
    use crate::game::tile::Tile;

    #[test]
    fn test_create_shuffle_deck() {
        // Create the shuffle deck
        let deck = create_deck();

        // Check that the deck has exactly 27 tiles
        assert_eq!(
            deck.tiles.len(),
            27,
            "The deck should contain exactly 27 tiles, but found {}.",
            deck.tiles.len()
        );

        // Verify that specific tiles exist in the deck
        let expected_tile = Tile(1, 2, 3);
        assert!(
            deck.tiles.contains(&expected_tile),
            "The deck should contain the tile {:?}.",
            expected_tile
        );

        let another_tile = Tile(9, 7, 8);
        assert!(
            deck.tiles.contains(&another_tile),
            "The deck should contain the tile {:?}.",
            another_tile
        );
    }

    #[test]
    fn test_new_tiles() {
        // Create a tile using new_tiles
        let tile = Tile(1, 2, 3);

        // Verify that the tile has the correct values
        assert_eq!(tile.0, 1, "The first value of the tile should be 1.");
        assert_eq!(tile.1, 2, "The second value of the tile should be 2.");
        assert_eq!(tile.2, 3, "The third value of the tile should be 3.");
    }
}
