#[cfg(test)]
mod tests {
    #[derive(Debug, Clone,PartialEq)]
    struct Plateau{
        tiles: Vec<Tile>,
    }
    #[derive(Debug, Clone,PartialEq)]
    struct Tile(i32, i32, i32);

    
    fn new_tiles(x: i32, y: i32, z: i32) -> Tile {
        Tile(x, y, z)
    }

    struct Deck{
    tiles: Vec<Tile>,
}
    fn create_shuffle_deck() -> Deck {
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

        Deck { tiles}
    }
    #[test]
    fn test_placement_tuile_valide_take_it_easy() {
        let mut plateau:Plateau=create_plateau_empty();
        let deckSfuffle:Deck= create_shuffle_deck();
        let tuile = deckSfuffle.tiles[5].clone();
        assert!(placer_tile(&mut plateau, tuile.clone(), 1));
        assert_eq!(plateau.tiles[1], tuile);
    }

    fn create_plateau_empty() -> Plateau {
        Plateau {
            tiles: vec![Tile(0, 0, 0); 19],
        }
    }


    fn placer_tile(plateau: &mut Plateau, tuile: Tile, position: usize) -> bool {
        if plateau.tiles[position] != Tile(0, 0, 0) {
            return false; // Case déjà occupée
        }
        plateau.tiles[position] = tuile;
        true
    }

}