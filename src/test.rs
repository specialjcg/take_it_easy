#[cfg(test)]
mod tests {
    use rand::Rng;

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

    fn create_plateau_empty() -> Plateau {
        Plateau {
            tiles: vec![Tile(0, 0, 0); 19],
        }
    }
    fn remove_tile_from_deck(deck: &Deck, tile_to_remove: &Tile) -> Deck {
        // Filtre toutes les tuiles sauf celle à retirer
        let new_tiles: Vec<Tile> = deck
            .tiles
            .iter()
            .filter(|&tile| tile != tile_to_remove) // Conserve uniquement les tuiles différentes
            .cloned() // Copie chaque tuile dans le nouveau vecteur
            .collect();

        Deck { tiles: new_tiles } // Crée un nouveau deck
    }


    fn placer_tile(plateau: &mut Plateau, tuile: Tile, position: usize) -> bool {
        if plateau.tiles[position] != Tile(0, 0, 0) {
            return false; // Case déjà occupée
        }
        plateau.tiles[position] = tuile;
        true
    }
    #[test]
    fn test_placement_tuile_valide_take_it_easy() {
        let mut plateau:Plateau=create_plateau_empty();
        let deckSfuffle:Deck= create_shuffle_deck();
        let tuile = deckSfuffle.tiles[5].clone();
        assert!(placer_tile(&mut plateau, tuile.clone(), 1));
        assert_eq!(plateau.tiles[1], tuile);
    }
    #[test]
    fn test_placement_tuile_not_valide_take_it_easy() {
        let mut plateau:Plateau=create_plateau_empty();
        let deckSfuffle:Deck= create_shuffle_deck();
        let tile = deckSfuffle.tiles[5].clone();
        assert!(placer_tile(&mut plateau, tile.clone(), 1));
        assert_eq!(plateau.tiles[1], tile);
        let tile = deckSfuffle.tiles[5].clone();
        assert!(!placer_tile(&mut plateau, tile.clone(), 1));
    }
    #[test]
    fn test_choix_aleatoire_tuile() {
        // Crée un deck
        let deck_shuffle: Deck = create_shuffle_deck();

        // Génère un index aléatoire
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..deck_shuffle.tiles.len());

        // Sélectionne une tuile aléatoire
        let tuile = deck_shuffle.tiles[index].clone();

        // Vérifie que la tuile existe dans le deck
        assert!(deck_shuffle.tiles.contains(&tuile));
        println!("Tuile choisie aléatoirement : {:?}", tuile);
    }
    #[test]
    fn test_retirer_tuile_aleatoire_du_deck() {
        use rand::Rng; // Pour générer un indice aléatoire

        // Crée un deck initial
        let deck_shuffle: Deck = create_shuffle_deck();

        // Génère un indice aléatoire
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..deck_shuffle.tiles.len());

        // Récupère la tuile choisie aléatoirement
        let tuile_choisie = deck_shuffle.tiles[index].clone();

        // Supprime la tuile du deck
        let nouveau_deck = remove_tile_from_deck(&deck_shuffle, &tuile_choisie);

        // Vérifie que la nouvelle taille du deck est réduite de 1
        assert_eq!(nouveau_deck.tiles.len(), deck_shuffle.tiles.len() - 1);

        // Vérifie que la tuile choisie n'est plus présente dans le nouveau deck
        assert!(!nouveau_deck.tiles.contains(&tuile_choisie));

        println!("Tuile retirée : {:?}", tuile_choisie);
        println!("Taille du deck initial : {}", deck_shuffle.tiles.len());
        println!("Taille du nouveau deck : {}", nouveau_deck.tiles.len());
    }

}